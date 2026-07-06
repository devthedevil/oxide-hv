use serde::Serialize;

use crate::host::HostId;
use crate::vm::VmId;

#[derive(Debug, Clone, Serialize)]
pub struct MigrationPlan {
    pub vm_id: VmId,
    pub source_host: HostId,
    pub target_host: HostId,
    pub estimated_total_ms: f64,
    pub estimated_downtime_ms: f64,
    pub precopy_rounds: u32,
}

/// Models a pre-copy live migration: the hypervisor transfers guest memory
/// over `link_bandwidth_mbps` while the VM keeps running and re-dirtying
/// pages. Each round only needs to re-send the pages dirtied during the
/// previous round; once the remaining dirty set is small enough (or a round
/// cap is hit) the VM is paused for a final stop-and-copy, which becomes the
/// guest-visible downtime.
pub fn plan_migration(mem_mb: u64, dirty_page_rate_mb_s: f64, link_bandwidth_mbps: f64) -> (f64, f64, u32) {
    let bandwidth_mb_s = link_bandwidth_mbps / 8.0;
    let max_rounds: u32 = 30;
    let stop_and_copy_threshold_mb = (bandwidth_mb_s * 0.05).max(1.0); // ~50ms worth of transfer left

    let mut remaining_mb = mem_mb as f64;
    let mut total_ms = 0.0;
    let mut round = 0;

    while round < max_rounds && remaining_mb > stop_and_copy_threshold_mb {
        let round_time_s = remaining_mb / bandwidth_mb_s;
        total_ms += round_time_s * 1000.0;
        remaining_mb = dirty_page_rate_mb_s * round_time_s;
        round += 1;
    }

    let downtime_ms = (remaining_mb / bandwidth_mb_s) * 1000.0;
    total_ms += downtime_ms;

    (total_ms, downtime_ms, round)
}

pub fn build_plan(vm_id: VmId, source_host: HostId, target_host: HostId, mem_mb: u64, dirty_page_rate_mb_s: f64, link_bandwidth_mbps: f64) -> MigrationPlan {
    let (total_ms, downtime_ms, rounds) = plan_migration(mem_mb, dirty_page_rate_mb_s, link_bandwidth_mbps);
    MigrationPlan {
        vm_id,
        source_host,
        target_host,
        estimated_total_ms: total_ms,
        estimated_downtime_ms: downtime_ms,
        precopy_rounds: rounds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_vm_migrates_with_near_zero_downtime() {
        let (_, downtime_ms, rounds) = plan_migration(4_096, 1.0, 10_000.0);
        assert!(downtime_ms < 5.0, "idle VM downtime should be tiny, got {downtime_ms}");
        assert!(rounds <= 2);
    }

    #[test]
    fn high_dirty_rate_increases_downtime_and_rounds() {
        let (_, idle_downtime, _) = plan_migration(65_536, 5.0, 10_000.0);
        let (_, busy_downtime, busy_rounds) = plan_migration(65_536, 900.0, 10_000.0);
        assert!(busy_downtime > idle_downtime, "hot VM should take longer to stop-and-copy");
        assert!(busy_rounds >= 1);
    }

    #[test]
    fn faster_link_reduces_total_migration_time() {
        let (slow_total, _, _) = plan_migration(16_384, 50.0, 1_000.0);
        let (fast_total, _, _) = plan_migration(16_384, 50.0, 25_000.0);
        assert!(fast_total < slow_total);
    }
}
