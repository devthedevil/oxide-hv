use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct ClusterMetrics {
    pub tick: u64,
    pub total_hosts: usize,
    pub healthy_hosts: usize,
    pub total_vms: usize,
    pub running_vms: usize,
    pub migrating_vms: usize,
    pub avg_cpu_utilization: f64,
    pub avg_mem_utilization: f64,
    pub avg_nvme_utilization: f64,
    pub gpu_utilization: f64,
    pub placement_failures_total: u64,
    pub scale_out_events_total: u64,
    pub scale_in_events_total: u64,
    pub migrations_total: u64,
}
