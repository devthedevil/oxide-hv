use std::collections::{HashMap, VecDeque};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;

use crate::autoscaler::{Autoscaler, AutoscalerConfig, ScalingEvent};
use crate::events::{Severity, SimEvent};
use crate::host::{Host, HostId};
use crate::metrics::ClusterMetrics;
use crate::migration::{self, MigrationPlan};
use crate::scheduler::{self, SchedulerError};
use crate::vm::{Vm, VmId, VmSpec, VmState};

const EVENT_LOG_CAPACITY: usize = 150;

#[derive(Debug, Clone, Copy)]
pub struct ClusterConfig {
    pub host_count: u32,
    pub numa_nodes_per_host: u8,
    pub cpu_millicores_per_numa: u32,
    pub mem_mb_per_numa: u64,
    pub gpus_per_host: u8,
    pub nvme_per_host: u8,
    pub nvme_max_iops: u32,
    pub link_bandwidth_mbps: f64,
    pub tick_duration_ms: f64,
    pub seed: u64,
    pub autoscaler: AutoscalerConfig,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            host_count: 12,
            numa_nodes_per_host: 2,
            cpu_millicores_per_numa: 16_000,
            mem_mb_per_numa: 131_072,
            gpus_per_host: 2,
            nvme_per_host: 2,
            nvme_max_iops: 500_000,
            link_bandwidth_mbps: 10_000.0,
            tick_duration_ms: 250.0,
            seed: 42,
            autoscaler: AutoscalerConfig::default(),
        }
    }
}

struct MigrationProgress {
    plan: MigrationPlan,
    numa_node: u8,
    elapsed_ms: f64,
}

pub struct Cluster {
    config: ClusterConfig,
    hosts: Vec<Host>,
    vms: HashMap<VmId, Vm>,
    active_migrations: HashMap<VmId, MigrationProgress>,
    anti_affinity_placements: Vec<(u32, HostId)>,
    autoscaler: Autoscaler,
    rng: StdRng,
    next_vm_id: VmId,
    tick_count: u64,
    events: VecDeque<SimEvent>,
    metrics: ClusterMetrics,
    scale_out_events_total: u64,
    scale_in_events_total: u64,
    migrations_total: u64,
    placement_failures_total: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClusterSnapshot<'a> {
    pub tick: u64,
    pub hosts: &'a [Host],
    pub vms: Vec<&'a Vm>,
    pub metrics: &'a ClusterMetrics,
    pub events: Vec<&'a SimEvent>,
}

impl Cluster {
    pub fn new(config: ClusterConfig) -> Self {
        let hosts: Vec<Host> = (0..config.host_count)
            .map(|id| {
                Host::new(
                    id,
                    config.numa_nodes_per_host,
                    config.cpu_millicores_per_numa,
                    config.mem_mb_per_numa,
                    config.gpus_per_host,
                    config.nvme_per_host,
                    config.nvme_max_iops,
                )
            })
            .collect();

        let mut cluster = Self {
            hosts,
            vms: HashMap::new(),
            active_migrations: HashMap::new(),
            anti_affinity_placements: Vec::new(),
            autoscaler: Autoscaler::new(config.autoscaler),
            rng: StdRng::seed_from_u64(config.seed),
            next_vm_id: 1,
            tick_count: 0,
            events: VecDeque::with_capacity(EVENT_LOG_CAPACITY),
            metrics: ClusterMetrics::default(),
            scale_out_events_total: 0,
            scale_in_events_total: 0,
            migrations_total: 0,
            placement_failures_total: 0,
            config,
        };

        cluster.log(
            Severity::Info,
            format!("cluster online: {} hosts, {} NUMA nodes each", cluster.config.host_count, cluster.config.numa_nodes_per_host),
        );
        cluster.seed_initial_load();
        cluster.recompute_metrics();
        cluster
    }

    fn seed_initial_load(&mut self) {
        let target_vms = (self.config.host_count as f64 * 3.5) as u32;
        for _ in 0..target_vms {
            let spec = self.random_spec();
            let _ = self.place_new_vm(spec);
        }
    }

    fn random_spec(&mut self) -> VmSpec {
        if self.rng.gen_bool(0.15) {
            VmSpec::gpu_heavy()
        } else {
            let jitter = self.rng.gen_range(80..=140);
            let mut spec = VmSpec::small();
            spec.vcpu_millicores = spec.vcpu_millicores * jitter / 100;
            spec.mem_mb = spec.mem_mb * jitter as u64 / 100;
            spec
        }
    }

    fn place_new_vm(&mut self, spec: VmSpec) -> Result<VmId, SchedulerError> {
        let decision = scheduler::find_placement(&self.hosts, &spec, &self.anti_affinity_placements)?;
        let vm_id = self.next_vm_id;
        self.next_vm_id += 1;

        if let Some(host) = self.hosts.iter_mut().find(|h| h.id == decision.host_id) {
            host.place(vm_id, decision.numa_node, &spec);
        }
        if let Some(group) = spec.anti_affinity_group {
            self.anti_affinity_placements.push((group, decision.host_id));
        }

        let dirty_rate = self.rng.gen_range(20.0..400.0);
        let vm = Vm::new(vm_id, spec, decision.host_id, decision.numa_node, dirty_rate);
        self.vms.insert(vm_id, vm);
        Ok(vm_id)
    }

    fn log(&mut self, severity: Severity, message: String) {
        if self.events.len() >= EVENT_LOG_CAPACITY {
            self.events.pop_front();
        }
        self.events.push_back(SimEvent {
            tick: self.tick_count,
            severity,
            message,
        });
    }

    /// Advances the simulation by one tick: progresses in-flight migrations,
    /// runs the autoscaler against current fleet utilization, and refreshes
    /// aggregate metrics.
    pub fn tick(&mut self) -> &ClusterMetrics {
        self.tick_count += 1;
        self.advance_migrations();
        self.run_autoscaler();
        self.recompute_metrics();
        &self.metrics
    }

    fn advance_migrations(&mut self) {
        let mut completed = Vec::new();
        for (vm_id, progress) in self.active_migrations.iter_mut() {
            progress.elapsed_ms += self.config.tick_duration_ms;
            let ratio = (progress.elapsed_ms / progress.plan.estimated_total_ms.max(1.0)).min(1.0) as f32;
            if let Some(vm) = self.vms.get_mut(vm_id) {
                vm.state = VmState::Migrating {
                    target_host: progress.plan.target_host,
                    progress: ratio,
                    downtime_ms: progress.plan.estimated_downtime_ms,
                };
            }
            if ratio >= 1.0 {
                completed.push(*vm_id);
            }
        }

        for vm_id in completed {
            if let Some(progress) = self.active_migrations.remove(&vm_id) {
                if let Some(vm) = self.vms.get_mut(&vm_id) {
                    let source = vm.host_id;
                    if let Some(src_host) = self.hosts.iter_mut().find(|h| h.id == source) {
                        src_host.release(vm_id, vm.numa_node, &vm.spec);
                    }
                    vm.host_id = progress.plan.target_host;
                    vm.numa_node = progress.numa_node;
                    vm.state = VmState::Running;
                    self.migrations_total += 1;
                    self.log(Severity::Info, format!("vm {vm_id} completed live migration -> host {}", progress.plan.target_host));
                }
            }
        }
    }

    fn run_autoscaler(&mut self) {
        let healthy: Vec<&Host> = self.hosts.iter().filter(|h| h.healthy).collect();
        if healthy.is_empty() {
            return;
        }
        let avg_cpu = healthy.iter().map(|h| h.cpu_utilization()).sum::<f64>() / healthy.len() as f64;
        let idle_vm = self.vms.values().find(|v| v.is_running() && v.spec.gpu_count == 0).map(|v| v.id);

        match self.autoscaler.evaluate(avg_cpu, idle_vm) {
            Some(ScalingEvent::ScaleOut { count }) => {
                let mut placed = 0;
                let mut failed = 0;
                for _ in 0..count {
                    let spec = self.random_spec();
                    match self.place_new_vm(spec) {
                        Ok(_) => placed += 1,
                        Err(_) => {
                            failed += 1;
                            self.placement_failures_total += 1;
                        }
                    }
                }
                self.scale_out_events_total += 1;
                if failed > 0 {
                    self.log(Severity::Warn, format!("autoscale-out: placed {placed}/{} VMs, cluster near capacity", placed + failed));
                } else {
                    self.log(Severity::Info, format!("autoscale-out: placed {placed} new VMs (avg CPU util {:.0}%)", avg_cpu * 100.0));
                }
            }
            Some(ScalingEvent::ScaleIn { vm_id }) => {
                self.terminate_vm(vm_id);
                self.scale_in_events_total += 1;
                self.log(Severity::Info, format!("autoscale-in: terminated idle vm {vm_id} (avg CPU util {:.0}%)", avg_cpu * 100.0));
            }
            None => {}
        }
    }

    fn terminate_vm(&mut self, vm_id: VmId) {
        if let Some(vm) = self.vms.remove(&vm_id) {
            if let Some(host) = self.hosts.iter_mut().find(|h| h.id == vm.host_id) {
                host.release(vm_id, vm.numa_node, &vm.spec);
            }
            self.anti_affinity_placements.retain(|(_, h)| *h != vm.host_id || vm.spec.anti_affinity_group.is_none());
        }
        self.active_migrations.remove(&vm_id);
    }

    /// Simulates a host failure: marks it unhealthy and evacuates every VM
    /// it was running via live migration to the least-loaded surviving
    /// host. VMs that can't be evacuated (insufficient cluster capacity)
    /// are marked terminated and logged as a critical event.
    pub fn fail_host(&mut self, host_id: HostId) {
        let Some(host) = self.hosts.iter_mut().find(|h| h.id == host_id) else { return };
        if !host.healthy {
            return;
        }
        host.healthy = false;
        let stranded: Vec<VmId> = host.vm_ids.clone();
        self.log(Severity::Critical, format!("host {host_id} failed — evacuating {} VMs", stranded.len()));

        for vm_id in stranded {
            self.start_migration(vm_id, None);
        }
    }

    pub fn recover_host(&mut self, host_id: HostId) {
        if let Some(host) = self.hosts.iter_mut().find(|h| h.id == host_id) {
            if !host.healthy {
                host.healthy = true;
                self.log(Severity::Info, format!("host {host_id} recovered and rejoined the pool"));
            }
        }
    }

    /// Starts a live migration for `vm_id`. If `target` is None, the
    /// scheduler picks the best surviving host. Resources on the target are
    /// reserved immediately (mirrors real hypervisors pre-allocating the
    /// destination before transferring guest memory).
    fn start_migration(&mut self, vm_id: VmId, target: Option<HostId>) {
        let Some(vm) = self.vms.get(&vm_id).cloned() else { return };
        if !vm.is_running() {
            return;
        }

        let decision = match target {
            Some(host_id) => self
                .hosts
                .iter()
                .find(|h| h.id == host_id)
                .and_then(|h| h.best_numa_fit(&vm.spec))
                .map(|numa| scheduler::PlacementDecision { host_id, numa_node: numa, score: 0.0 }),
            None => scheduler::find_placement(&self.hosts, &vm.spec, &self.anti_affinity_placements).ok(),
        };

        let Some(decision) = decision else {
            self.vms.remove(&vm_id);
            self.log(Severity::Critical, format!("vm {vm_id} could not be evacuated — no capacity, VM lost"));
            return;
        };

        if let Some(dest) = self.hosts.iter_mut().find(|h| h.id == decision.host_id) {
            dest.place(vm_id, decision.numa_node, &vm.spec);
        }

        let plan = migration::build_plan(vm_id, vm.host_id, decision.host_id, vm.spec.mem_mb, vm.dirty_page_rate_mb_s, self.config.link_bandwidth_mbps);
        self.log(
            Severity::Info,
            format!("vm {vm_id} live-migrating {} -> {} (est. downtime {:.1}ms)", vm.host_id, decision.host_id, plan.estimated_downtime_ms),
        );
        self.active_migrations.insert(
            vm_id,
            MigrationProgress {
                plan,
                numa_node: decision.numa_node,
                elapsed_ms: 0.0,
            },
        );

        if let Some(vm) = self.vms.get_mut(&vm_id) {
            vm.state = VmState::Migrating {
                target_host: decision.host_id,
                progress: 0.0,
                downtime_ms: 0.0,
            };
        }
    }

    /// Manually injects a burst of new VM placement requests (e.g. a "load
    /// spike" button on the dashboard), independent of the autoscaler.
    pub fn inject_burst(&mut self, count: u32) {
        let mut placed = 0;
        for _ in 0..count {
            let spec = self.random_spec();
            if self.place_new_vm(spec).is_ok() {
                placed += 1;
            } else {
                self.placement_failures_total += 1;
            }
        }
        self.log(Severity::Warn, format!("load burst injected: placed {placed}/{count} VMs"));
    }

    fn recompute_metrics(&mut self) {
        let healthy_hosts: Vec<&Host> = self.hosts.iter().filter(|h| h.healthy).collect();
        let n = healthy_hosts.len().max(1) as f64;

        let avg_cpu = healthy_hosts.iter().map(|h| h.cpu_utilization()).sum::<f64>() / n;
        let avg_mem = healthy_hosts.iter().map(|h| h.mem_utilization()).sum::<f64>() / n;
        let avg_nvme = healthy_hosts.iter().map(|h| h.nvme_utilization()).sum::<f64>() / n;
        let total_gpus: usize = healthy_hosts.iter().map(|h| h.gpus.len()).sum();
        let used_gpus: usize = healthy_hosts.iter().map(|h| h.gpus.len() - h.free_gpus()).sum();

        let migrating_vms = self.vms.values().filter(|v| matches!(v.state, VmState::Migrating { .. })).count();

        self.metrics = ClusterMetrics {
            tick: self.tick_count,
            total_hosts: self.hosts.len(),
            healthy_hosts: healthy_hosts.len(),
            total_vms: self.vms.len(),
            running_vms: self.vms.len() - migrating_vms,
            migrating_vms,
            avg_cpu_utilization: avg_cpu,
            avg_mem_utilization: avg_mem,
            avg_nvme_utilization: avg_nvme,
            gpu_utilization: used_gpus as f64 / total_gpus.max(1) as f64,
            placement_failures_total: self.placement_failures_total,
            scale_out_events_total: self.scale_out_events_total,
            scale_in_events_total: self.scale_in_events_total,
            migrations_total: self.migrations_total,
        };
    }

    pub fn metrics(&self) -> &ClusterMetrics {
        &self.metrics
    }

    pub fn hosts(&self) -> &[Host] {
        &self.hosts
    }

    pub fn snapshot(&self) -> ClusterSnapshot<'_> {
        ClusterSnapshot {
            tick: self.tick_count,
            hosts: &self.hosts,
            vms: self.vms.values().collect(),
            metrics: &self.metrics,
            events: self.events.iter().collect(),
        }
    }
}
