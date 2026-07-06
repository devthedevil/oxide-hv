export interface NumaNode {
  id: number;
  cpu_capacity_millicores: number;
  cpu_used_millicores: number;
  mem_capacity_mb: number;
  mem_used_mb: number;
}

export interface GpuDevice {
  id: number;
  model: string;
  assigned_to: number | null;
}

export interface NvmeDevice {
  id: number;
  max_iops: number;
  used_iops: number;
}

export interface Host {
  id: number;
  healthy: boolean;
  numa_nodes: NumaNode[];
  gpus: GpuDevice[];
  nvme_devices: NvmeDevice[];
  vm_ids: number[];
}

export interface VmSpec {
  vcpu_millicores: number;
  mem_mb: number;
  gpu_count: number;
  nvme_iops: number;
  anti_affinity_group: number | null;
}

export type VmState =
  | { kind: "Running" }
  | { kind: "Migrating"; target_host: number; progress: number; downtime_ms: number }
  | { kind: "Terminated" };

export interface Vm {
  id: number;
  spec: VmSpec;
  host_id: number;
  numa_node: number;
  state: VmState;
  dirty_page_rate_mb_s: number;
}

export interface ClusterMetrics {
  tick: number;
  total_hosts: number;
  healthy_hosts: number;
  total_vms: number;
  running_vms: number;
  migrating_vms: number;
  avg_cpu_utilization: number;
  avg_mem_utilization: number;
  avg_nvme_utilization: number;
  gpu_utilization: number;
  placement_failures_total: number;
  scale_out_events_total: number;
  scale_in_events_total: number;
  migrations_total: number;
}

export type Severity = "Info" | "Warn" | "Critical";

export interface SimEvent {
  tick: number;
  severity: Severity;
  message: string;
}

export interface ClusterSnapshot {
  tick: number;
  hosts: Host[];
  vms: Vm[];
  metrics: ClusterMetrics;
  events: SimEvent[];
}
