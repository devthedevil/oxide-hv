//! Deterministic simulation engine for a NUMA-aware hypervisor scheduler.
//!
//! Models a fleet of hosts exposing CPUs (grouped into NUMA nodes), GPUs
//! (exclusive passthrough), and NVMe controllers (IOPS-limited), and
//! simulates: bin-packing VM placement, pre-copy live migration with a
//! dirty-page cost model, host-failure evacuation, and threshold-based
//! elastic autoscaling.

pub mod autoscaler;
pub mod cluster;
pub mod events;
pub mod host;
pub mod metrics;
pub mod migration;
pub mod scheduler;
pub mod vm;

pub use autoscaler::{Autoscaler, AutoscalerConfig, ScalingEvent};
pub use cluster::{Cluster, ClusterConfig, ClusterSnapshot};
pub use events::{Severity, SimEvent};
pub use host::{GpuDevice, Host, HostId, NumaNode, NvmeDevice};
pub use metrics::ClusterMetrics;
pub use migration::MigrationPlan;
pub use scheduler::{PlacementDecision, SchedulerError};
pub use vm::{Vm, VmId, VmSpec, VmState};
