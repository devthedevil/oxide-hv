use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use oxide_engine::{Cluster, ClusterConfig};

fn bench_tick_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("cluster_tick");
    for host_count in [16u32, 64, 256] {
        let config = ClusterConfig {
            host_count,
            ..ClusterConfig::default()
        };
        group.bench_with_input(BenchmarkId::from_parameter(host_count), &config, |b, config| {
            let mut cluster = Cluster::new(*config);
            b.iter(|| {
                black_box(cluster.tick());
            });
        });
    }
    group.finish();
}

fn bench_load_burst_placement(c: &mut Criterion) {
    let mut group = c.benchmark_group("inject_burst");
    for burst in [50u32, 500] {
        group.bench_with_input(BenchmarkId::from_parameter(burst), &burst, |b, &burst| {
            b.iter_batched(
                || {
                    Cluster::new(ClusterConfig {
                        host_count: 128,
                        ..ClusterConfig::default()
                    })
                },
                |mut cluster| {
                    cluster.inject_burst(burst);
                    black_box(cluster.metrics().total_vms)
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_tick_throughput, bench_load_burst_placement);
criterion_main!(benches);
