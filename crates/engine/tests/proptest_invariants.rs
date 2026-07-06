use oxide_engine::{Cluster, ClusterConfig};
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum Action {
    Tick,
    Burst(u32),
    Fail(u32),
    Recover(u32),
}

fn action_strategy() -> impl Strategy<Value = Action> {
    prop_oneof![
        3 => Just(Action::Tick),
        2 => (0u32..10).prop_map(Action::Burst),
        1 => (0u32..6).prop_map(Action::Fail),
        1 => (0u32..6).prop_map(Action::Recover),
    ]
}

/// After any sequence of ticks, load bursts, and host failures/recoveries,
/// every host's resource accounting must stay within physical capacity —
/// the scheduler and migration paths must never double-book a NUMA node,
/// GPU, or NVMe device.
fn assert_capacity_invariants(cluster: &Cluster) {
    for host in cluster.hosts() {
        for numa in &host.numa_nodes {
            assert!(
                numa.cpu_used_millicores <= numa.cpu_capacity_millicores,
                "host {} numa {} over-committed CPU: {} > {}",
                host.id,
                numa.id,
                numa.cpu_used_millicores,
                numa.cpu_capacity_millicores
            );
            assert!(
                numa.mem_used_mb <= numa.mem_capacity_mb,
                "host {} numa {} over-committed memory: {} > {}",
                host.id,
                numa.id,
                numa.mem_used_mb,
                numa.mem_capacity_mb
            );
        }

        let assigned_gpus = host.gpus.iter().filter(|g| g.assigned_to.is_some()).count();
        assert!(assigned_gpus <= host.gpus.len(), "host {} assigned more GPUs than it physically has", host.id);

        for dev in &host.nvme_devices {
            assert!(
                dev.used_iops <= dev.max_iops,
                "host {} nvme {} over-committed IOPS: {} > {}",
                host.id,
                dev.id,
                dev.used_iops,
                dev.max_iops
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn capacity_never_oversubscribed(actions in prop::collection::vec(action_strategy(), 1..80)) {
        let mut cluster = Cluster::new(ClusterConfig { host_count: 6, ..ClusterConfig::default() });
        assert_capacity_invariants(&cluster);

        for action in actions {
            match action {
                Action::Tick => { cluster.tick(); }
                Action::Burst(n) => cluster.inject_burst(n),
                Action::Fail(id) => cluster.fail_host(id),
                Action::Recover(id) => cluster.recover_host(id),
            }
            assert_capacity_invariants(&cluster);
        }
    }
}
