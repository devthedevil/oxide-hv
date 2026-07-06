use rayon::prelude::*;
use thiserror::Error;

use crate::host::{Host, HostId};
use crate::vm::{VmId, VmSpec};

#[derive(Debug, Error, PartialEq)]
pub enum SchedulerError {
    #[error("no host has sufficient CPU/memory/GPU/NVMe headroom for the requested spec")]
    InsufficientCapacity,
    #[error("placing this VM would violate anti-affinity group {0}")]
    AntiAffinityViolation(u32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacementDecision {
    pub host_id: HostId,
    pub numa_node: u8,
    pub score: f64,
}

/// Weighted best-fit scoring: prefers hosts with balanced headroom across
/// CPU/mem/NVMe rather than draining a single dimension first, which keeps
/// the cluster able to accept mixed workload shapes longer (a classic
/// bin-packing fragmentation avoidance heuristic).
fn score_host(host: &Host, spec: &VmSpec) -> Option<(u8, f64)> {
    let numa_node = host.best_numa_fit(spec)?;

    let cpu_headroom_ratio = 1.0 - host.cpu_utilization();
    let mem_headroom_ratio = 1.0 - host.mem_utilization();
    let nvme_headroom_ratio = 1.0 - host.nvme_utilization();
    let balance_penalty = (cpu_headroom_ratio - mem_headroom_ratio).abs();

    let score = 0.4 * cpu_headroom_ratio + 0.35 * mem_headroom_ratio + 0.25 * nvme_headroom_ratio - 0.15 * balance_penalty;

    Some((numa_node, score))
}

fn violates_anti_affinity(host: &Host, spec: &VmSpec, group_hosts: &[(u32, HostId)]) -> bool {
    match spec.anti_affinity_group {
        Some(group) => group_hosts.iter().any(|(g, h)| *g == group && *h == host.id),
        None => false,
    }
}

/// Finds the best placement for a single VM spec across all healthy hosts.
/// Host scoring runs in parallel via rayon so a scheduling decision over a
/// large fleet stays cheap even under autoscale bursts placing hundreds of
/// VMs per tick.
pub fn find_placement(hosts: &[Host], spec: &VmSpec, group_hosts: &[(u32, HostId)]) -> Result<PlacementDecision, SchedulerError> {
    let candidate = hosts
        .par_iter()
        .filter(|h| h.healthy)
        .filter(|h| !violates_anti_affinity(h, spec, group_hosts))
        .filter_map(|h| {
            score_host(h, spec).map(|(numa, score)| PlacementDecision {
                host_id: h.id,
                numa_node: numa,
                score,
            })
        })
        .max_by(|a, b| a.score.partial_cmp(&b.score).unwrap());

    match candidate {
        Some(decision) => Ok(decision),
        None => {
            let any_fits_ignoring_affinity = hosts.iter().filter(|h| h.healthy).any(|h| score_host(h, spec).is_some());
            match spec.anti_affinity_group {
                Some(group) if any_fits_ignoring_affinity => Err(SchedulerError::AntiAffinityViolation(group)),
                _ => Err(SchedulerError::InsufficientCapacity),
            }
        }
    }
}

/// Places a batch of specs (e.g. an autoscale burst), scheduling each
/// against the cumulative effect of prior placements in the batch so the
/// scorer doesn't pile every VM onto the single best host.
pub fn find_placements_batch(hosts: &mut [Host], specs: &[(VmId, VmSpec)], group_hosts: &mut Vec<(u32, HostId)>) -> Vec<(VmId, Result<PlacementDecision, SchedulerError>)> {
    let mut results = Vec::with_capacity(specs.len());
    for (vm_id, spec) in specs {
        let decision = find_placement(hosts, spec, group_hosts);
        if let Ok(d) = &decision {
            if let Some(group) = spec.anti_affinity_group {
                group_hosts.push((group, d.host_id));
            }
            if let Some(host) = hosts.iter_mut().find(|h| h.id == d.host_id) {
                host.place(*vm_id, d.numa_node, spec);
            }
        }
        results.push((*vm_id, decision));
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::Host;

    fn hosts(n: u32) -> Vec<Host> {
        (0..n).map(|id| Host::new(id, 1, 8_000, 16_384, 1, 1, 10_000)).collect()
    }

    #[test]
    fn rejects_when_no_host_has_capacity() {
        let hosts = hosts(1);
        let spec = VmSpec {
            vcpu_millicores: 100_000,
            mem_mb: 1_024,
            gpu_count: 0,
            nvme_iops: 0,
            anti_affinity_group: None,
        };
        assert_eq!(find_placement(&hosts, &spec, &[]), Err(SchedulerError::InsufficientCapacity));
    }

    #[test]
    fn skips_unhealthy_hosts() {
        let mut hosts = hosts(2);
        hosts[0].healthy = false;
        let spec = VmSpec::small();
        let decision = find_placement(&hosts, &spec, &[]).unwrap();
        assert_eq!(decision.host_id, 1);
    }

    #[test]
    fn anti_affinity_forces_different_hosts() {
        let mut hosts = hosts(2);
        let spec = VmSpec {
            vcpu_millicores: 1_000,
            mem_mb: 1_024,
            gpu_count: 0,
            nvme_iops: 0,
            anti_affinity_group: Some(7),
        };

        let first = find_placement(&hosts, &spec, &[]).unwrap();
        hosts.iter_mut().find(|h| h.id == first.host_id).unwrap().place(1, first.numa_node, &spec);
        let group_hosts = vec![(7, first.host_id)];

        let second = find_placement(&hosts, &spec, &group_hosts).unwrap();
        assert_ne!(second.host_id, first.host_id);
    }

    #[test]
    fn anti_affinity_violation_reported_when_only_conflicting_host_has_room() {
        let hosts = hosts(1);
        let spec = VmSpec {
            vcpu_millicores: 1_000,
            mem_mb: 1_024,
            gpu_count: 0,
            nvme_iops: 0,
            anti_affinity_group: Some(1),
        };
        let group_hosts = vec![(1, 0)];
        assert_eq!(find_placement(&hosts, &spec, &group_hosts), Err(SchedulerError::AntiAffinityViolation(1)));
    }

    #[test]
    fn batch_placement_spreads_load_across_hosts() {
        let mut hosts = hosts(4);
        let specs: Vec<(VmId, VmSpec)> = (0..4).map(|i| (i as VmId, VmSpec::small())).collect();
        let mut group_hosts = Vec::new();
        let results = find_placements_batch(&mut hosts, &specs, &mut group_hosts);
        assert!(results.iter().all(|(_, r)| r.is_ok()));
        let distinct_hosts: std::collections::HashSet<_> = results.iter().map(|(_, r)| r.as_ref().unwrap().host_id).collect();
        assert!(distinct_hosts.len() > 1, "batch scoring should not pile every VM onto a single host");
    }
}
