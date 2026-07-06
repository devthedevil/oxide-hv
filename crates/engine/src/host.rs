use serde::Serialize;

use crate::vm::{VmId, VmSpec};

pub type HostId = u32;

#[derive(Debug, Clone, Serialize)]
pub struct NumaNode {
    pub id: u8,
    pub cpu_capacity_millicores: u32,
    pub cpu_used_millicores: u32,
    pub mem_capacity_mb: u64,
    pub mem_used_mb: u64,
}

impl NumaNode {
    pub fn cpu_headroom(&self) -> u32 {
        self.cpu_capacity_millicores.saturating_sub(self.cpu_used_millicores)
    }

    pub fn mem_headroom(&self) -> u64 {
        self.mem_capacity_mb.saturating_sub(self.mem_used_mb)
    }

    pub fn cpu_utilization(&self) -> f64 {
        self.cpu_used_millicores as f64 / self.cpu_capacity_millicores.max(1) as f64
    }
}

/// A PCIe GPU exposed via passthrough. Exclusive assignment only (no vGPU
/// time-slicing) mirrors how accelerators are typically handed to tenants
/// that need near bare-metal performance.
#[derive(Debug, Clone, Serialize)]
pub struct GpuDevice {
    pub id: u8,
    pub model: &'static str,
    pub assigned_to: Option<VmId>,
}

/// A virtualized NVMe controller. `used_iops` models queue contention: as
/// tenants approach `max_iops` the effective per-VM latency the dashboard
/// reports climbs, mirroring queue-depth backpressure on real NVMe stacks.
#[derive(Debug, Clone, Serialize)]
pub struct NvmeDevice {
    pub id: u8,
    pub max_iops: u32,
    pub used_iops: u32,
}

impl NvmeDevice {
    pub fn headroom_iops(&self) -> u32 {
        self.max_iops.saturating_sub(self.used_iops)
    }

    pub fn utilization(&self) -> f64 {
        self.used_iops as f64 / self.max_iops.max(1) as f64
    }

    /// Effective read/write latency model: latency stays flat under ~70%
    /// queue utilization then rises sharply as the device saturates.
    pub fn estimated_latency_us(&self) -> f64 {
        let u = self.utilization().min(0.999);
        let base_latency_us = 80.0;
        base_latency_us / (1.0 - u).max(0.001)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Host {
    pub id: HostId,
    pub healthy: bool,
    pub numa_nodes: Vec<NumaNode>,
    pub gpus: Vec<GpuDevice>,
    pub nvme_devices: Vec<NvmeDevice>,
    pub vm_ids: Vec<VmId>,
}

impl Host {
    pub fn new(id: HostId, numa_node_count: u8, cpu_per_numa: u32, mem_per_numa: u64, gpu_count: u8, nvme_count: u8, nvme_max_iops: u32) -> Self {
        let numa_nodes = (0..numa_node_count)
            .map(|n| NumaNode {
                id: n,
                cpu_capacity_millicores: cpu_per_numa,
                cpu_used_millicores: 0,
                mem_capacity_mb: mem_per_numa,
                mem_used_mb: 0,
            })
            .collect();
        let gpus = (0..gpu_count)
            .map(|g| GpuDevice {
                id: g,
                model: "A10-24GB",
                assigned_to: None,
            })
            .collect();
        let nvme_devices = (0..nvme_count)
            .map(|n| NvmeDevice {
                id: n,
                max_iops: nvme_max_iops,
                used_iops: 0,
            })
            .collect();
        Self {
            id,
            healthy: true,
            numa_nodes,
            gpus,
            nvme_devices,
            vm_ids: Vec::new(),
        }
    }

    pub fn total_cpu_capacity(&self) -> u32 {
        self.numa_nodes.iter().map(|n| n.cpu_capacity_millicores).sum()
    }

    pub fn total_cpu_used(&self) -> u32 {
        self.numa_nodes.iter().map(|n| n.cpu_used_millicores).sum()
    }

    pub fn cpu_utilization(&self) -> f64 {
        self.total_cpu_used() as f64 / self.total_cpu_capacity().max(1) as f64
    }

    pub fn total_mem_capacity(&self) -> u64 {
        self.numa_nodes.iter().map(|n| n.mem_capacity_mb).sum()
    }

    pub fn total_mem_used(&self) -> u64 {
        self.numa_nodes.iter().map(|n| n.mem_used_mb).sum()
    }

    pub fn mem_utilization(&self) -> f64 {
        self.total_mem_used() as f64 / self.total_mem_capacity().max(1) as f64
    }

    pub fn free_gpus(&self) -> usize {
        self.gpus.iter().filter(|g| g.assigned_to.is_none()).count()
    }

    pub fn nvme_headroom_iops(&self) -> u32 {
        self.nvme_devices.iter().map(|d| d.headroom_iops()).sum()
    }

    pub fn nvme_utilization(&self) -> f64 {
        let cap: u32 = self.nvme_devices.iter().map(|d| d.max_iops).sum();
        let used: u32 = self.nvme_devices.iter().map(|d| d.used_iops).sum();
        used as f64 / cap.max(1) as f64
    }

    /// Best NUMA node to satisfy `spec` on this host, if any node (plus the
    /// host's GPU/NVMe pools) has enough headroom.
    pub fn best_numa_fit(&self, spec: &VmSpec) -> Option<u8> {
        if self.free_gpus() < spec.gpu_count as usize {
            return None;
        }
        if self.nvme_headroom_iops() < spec.nvme_iops {
            return None;
        }
        self.numa_nodes
            .iter()
            .filter(|n| n.cpu_headroom() >= spec.vcpu_millicores && n.mem_headroom() >= spec.mem_mb)
            .max_by(|a, b| a.cpu_headroom().cmp(&b.cpu_headroom()))
            .map(|n| n.id)
    }

    pub fn place(&mut self, vm_id: VmId, numa_node: u8, spec: &VmSpec) {
        if let Some(node) = self.numa_nodes.iter_mut().find(|n| n.id == numa_node) {
            node.cpu_used_millicores += spec.vcpu_millicores;
            node.mem_used_mb += spec.mem_mb;
        }
        let mut remaining_gpu = spec.gpu_count;
        for gpu in self.gpus.iter_mut() {
            if remaining_gpu == 0 {
                break;
            }
            if gpu.assigned_to.is_none() {
                gpu.assigned_to = Some(vm_id);
                remaining_gpu -= 1;
            }
        }
        let mut remaining_iops = spec.nvme_iops;
        for dev in self.nvme_devices.iter_mut() {
            if remaining_iops == 0 {
                break;
            }
            let take = remaining_iops.min(dev.headroom_iops());
            dev.used_iops += take;
            remaining_iops -= take;
        }
        self.vm_ids.push(vm_id);
    }

    pub fn release(&mut self, vm_id: VmId, numa_node: u8, spec: &VmSpec) {
        if let Some(node) = self.numa_nodes.iter_mut().find(|n| n.id == numa_node) {
            node.cpu_used_millicores = node.cpu_used_millicores.saturating_sub(spec.vcpu_millicores);
            node.mem_used_mb = node.mem_used_mb.saturating_sub(spec.mem_mb);
        }
        for gpu in self.gpus.iter_mut().filter(|g| g.assigned_to == Some(vm_id)) {
            gpu.assigned_to = None;
        }
        let mut remaining_iops = spec.nvme_iops;
        for dev in self.nvme_devices.iter_mut() {
            if remaining_iops == 0 {
                break;
            }
            let give_back = remaining_iops.min(dev.used_iops);
            dev.used_iops -= give_back;
            remaining_iops -= give_back;
        }
        self.vm_ids.retain(|id| *id != vm_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_host() -> Host {
        Host::new(0, 2, 4_000, 8_192, 1, 1, 10_000)
    }

    #[test]
    fn place_then_release_returns_to_baseline() {
        let mut host = test_host();
        let spec = VmSpec {
            vcpu_millicores: 2_000,
            mem_mb: 4_096,
            gpu_count: 1,
            nvme_iops: 5_000,
            anti_affinity_group: None,
        };
        let node = host.best_numa_fit(&spec).expect("should fit");
        host.place(1, node, &spec);
        assert_eq!(host.total_cpu_used(), 2_000);
        assert_eq!(host.free_gpus(), 0);
        assert_eq!(host.nvme_headroom_iops(), 5_000);

        host.release(1, node, &spec);
        assert_eq!(host.total_cpu_used(), 0);
        assert_eq!(host.free_gpus(), 1);
        assert_eq!(host.nvme_headroom_iops(), 10_000);
    }

    #[test]
    fn best_numa_fit_rejects_oversized_gpu_request() {
        let host = test_host();
        let spec = VmSpec {
            vcpu_millicores: 1_000,
            mem_mb: 1_024,
            gpu_count: 5,
            nvme_iops: 100,
            anti_affinity_group: None,
        };
        assert!(host.best_numa_fit(&spec).is_none());
    }

    #[test]
    fn nvme_latency_climbs_as_queue_saturates() {
        let mut dev = NvmeDevice { id: 0, max_iops: 1_000, used_iops: 0 };
        let idle_latency = dev.estimated_latency_us();
        dev.used_iops = 950;
        let saturated_latency = dev.estimated_latency_us();
        assert!(saturated_latency > idle_latency * 5.0);
    }
}
