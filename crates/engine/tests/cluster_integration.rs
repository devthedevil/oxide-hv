use oxide_engine::{Cluster, ClusterConfig, VmState};

#[test]
fn cluster_boots_with_initial_load_and_healthy_hosts() {
    let cluster = Cluster::new(ClusterConfig::default());
    let metrics = cluster.metrics();
    assert_eq!(metrics.healthy_hosts, metrics.total_hosts);
    assert!(metrics.total_vms > 0, "seed_initial_load should have placed VMs");
}

#[test]
fn ticking_advances_without_panicking_and_updates_tick_counter() {
    let mut cluster = Cluster::new(ClusterConfig::default());
    for _ in 0..50 {
        cluster.tick();
    }
    assert_eq!(cluster.metrics().tick, 50);
}

#[test]
fn host_failure_evacuates_vms_via_live_migration() {
    let mut cluster = Cluster::new(ClusterConfig {
        host_count: 8,
        ..ClusterConfig::default()
    });
    let victim_host = cluster.hosts()[0].id;
    let vms_before_on_victim = cluster.hosts()[0].vm_ids.len();
    assert!(vms_before_on_victim > 0, "expected the victim host to be running VMs before failure");

    cluster.fail_host(victim_host);
    assert!(!cluster.hosts().iter().find(|h| h.id == victim_host).unwrap().healthy);

    let snapshot = cluster.snapshot();
    let migrating = snapshot.vms.iter().filter(|v| matches!(v.state, VmState::Migrating { .. })).count();
    assert!(migrating > 0, "evacuated VMs should be in the Migrating state immediately after failure");

    for _ in 0..200 {
        cluster.tick();
    }

    let still_on_victim = cluster.hosts().iter().find(|h| h.id == victim_host).unwrap().vm_ids.len();
    assert_eq!(still_on_victim, 0, "failed host should have no VMs left after evacuation completes");
}

#[test]
fn recovering_a_failed_host_makes_it_schedulable_again() {
    let mut cluster = Cluster::new(ClusterConfig::default());
    let host_id = cluster.hosts()[0].id;
    cluster.fail_host(host_id);
    cluster.recover_host(host_id);
    assert!(cluster.hosts().iter().find(|h| h.id == host_id).unwrap().healthy);
}

#[test]
fn inject_burst_increases_vm_count_up_to_capacity() {
    let mut cluster = Cluster::new(ClusterConfig {
        host_count: 4,
        ..ClusterConfig::default()
    });
    let before = cluster.metrics().total_vms;
    cluster.inject_burst(5);
    cluster.tick();
    assert!(cluster.metrics().total_vms >= before);
}
