use serde::{Deserialize, Serialize};

use crate::host::HostId;

pub type VmId = u64;

/// Resource request a tenant workload asks the scheduler to place.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct VmSpec {
    pub vcpu_millicores: u32,
    pub mem_mb: u64,
    pub gpu_count: u8,
    pub nvme_iops: u32,
    /// VMs sharing a group id are placed on different hosts (anti-affinity).
    pub anti_affinity_group: Option<u32>,
}

impl VmSpec {
    pub fn small() -> Self {
        Self {
            vcpu_millicores: 1_000,
            mem_mb: 4_096,
            gpu_count: 0,
            nvme_iops: 2_000,
            anti_affinity_group: None,
        }
    }

    pub fn gpu_heavy() -> Self {
        Self {
            vcpu_millicores: 8_000,
            mem_mb: 65_536,
            gpu_count: 1,
            nvme_iops: 20_000,
            anti_affinity_group: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
#[serde(tag = "kind")]
pub enum VmState {
    Running,
    Migrating { target_host: HostId, progress: f32, downtime_ms: f64 },
    Terminated,
}

#[derive(Debug, Clone, Serialize)]
pub struct Vm {
    pub id: VmId,
    pub spec: VmSpec,
    pub host_id: HostId,
    pub numa_node: u8,
    pub state: VmState,
    /// Simulated guest memory dirty rate, drives the live-migration cost model.
    pub dirty_page_rate_mb_s: f64,
}

impl Vm {
    pub fn new(id: VmId, spec: VmSpec, host_id: HostId, numa_node: u8, dirty_page_rate_mb_s: f64) -> Self {
        Self {
            id,
            spec,
            host_id,
            numa_node,
            state: VmState::Running,
            dirty_page_rate_mb_s,
        }
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, VmState::Running)
    }
}
