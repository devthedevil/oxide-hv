# OxideHV

A deterministic simulation of a NUMA-aware hypervisor scheduler — VM placement, live migration, GPU/NVMe device virtualization, host-failure evacuation, and elastic autoscaling — written in Rust, compiled to WebAssembly, and rendered live in a Next.js dashboard.

**[Live demo →](https://oxide-hv.vercel.app)** _(replace with your deployed URL)_

## Why this exists

Real hypervisor control planes (the layer that decides which physical host runs which VM, and how to move a VM without the guest noticing) are hard to observe directly — you can't easily watch a datacenter rebalance itself. OxideHV models the same core problems in miniature and lets you poke at them interactively: kill a host and watch live migration evacuate it, dump a load spike on the cluster and watch the autoscaler and bin-packer respond, and see how NVMe queue contention and GPU passthrough scarcity affect where a VM can land.

The whole simulation runs client-side — the Rust engine compiles to a `.wasm` module loaded directly by the browser, with no backend server required.

## Architecture

```
crates/engine/   pure-Rust simulation core (no I/O, fully unit-testable)
crates/wasm/     wasm-bindgen bindings exposing the engine to JS
web/             Next.js dashboard that drives the simulation loop and renders it
```

### Simulation model (`crates/engine`)

- **Hosts** expose CPU/memory grouped into NUMA nodes, a fixed number of passthrough GPUs, and NVMe controllers with an IOPS ceiling.
- **Scheduler** (`scheduler.rs`) scores every healthy host in parallel (via `rayon`) using a weighted best-fit heuristic across CPU/memory/NVMe headroom, penalizing hosts whose resource dimensions are imbalanced — a classic bin-packing fragmentation-avoidance technique. Anti-affinity groups are enforced so tenants can spread replicas across hosts.
- **Live migration** (`migration.rs`) models iterative pre-copy: each round only needs to retransmit memory dirtied during the previous round, so total migration time and guest-visible downtime both fall out of the VM's memory size, its dirty-page rate, and the network link's bandwidth. A busy VM takes longer to converge and pays a larger final stop-and-copy pause.
- **Host failure** (`cluster::fail_host`) marks a host unhealthy and evacuates every VM it held via the same live-migration path used for voluntary rebalancing; a VM is only lost if the rest of the fleet genuinely has no room for it.
- **Autoscaler** (`autoscaler.rs`) watches fleet-average CPU utilization and scales the VM count up or down against configurable thresholds, with a cooldown window so it doesn't thrash on transient spikes.
- **NVMe latency model** (`host.rs`) keeps effective latency flat under moderate queue utilization and climbs sharply as a device saturates, mirroring real queue-depth backpressure.

Every mutation goes through `Host::place`/`Host::release`, so a property test (`tests/proptest_invariants.rs`) can throw thousands of randomized action sequences — ticks, load bursts, host failures, recoveries — at the cluster and assert that CPU, memory, GPU, and NVMe accounting can never be oversubscribed, regardless of ordering.

### WASM boundary (`crates/wasm`)

A single `SimHandle` type exposes `tick()`, `snapshot()`, `fail_host()`, `recover_host()`, and `inject_burst()` to JavaScript. `snapshot()` serializes the entire cluster state (hosts, VMs, metrics, event log) to a JS value via `serde-wasm-bindgen` in one call, so the dashboard never has to round-trip individual fields.

### Dashboard (`web/`)

A client-side render loop calls `tick()` + `snapshot()` on an interval and renders host utilization grids, fleet-wide metrics, and a live event log. The compiled `wasm-pack --target web` output is committed under `web/public/pkg` and loaded with a runtime (non-bundled) dynamic `import()`, so no wasm toolchain is required at deploy time — Vercel just serves static files.

## Running locally

```bash
# 1. Rust engine: format, lint, test, benchmark
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo bench -p oxide-engine

# 2. Rebuild the wasm package after touching crates/engine or crates/wasm
./scripts/wasm-pkg.sh build

# 3. Run the dashboard
cd web
npm install
npm run dev   # http://localhost:3000
```

## Testing strategy

- **Unit tests** per module (`host`, `scheduler`, `migration`, `autoscaler`) covering placement rejection, anti-affinity, migration cost scaling with dirty-page rate and link bandwidth, and autoscaler cooldown behavior.
- **Integration tests** (`crates/engine/tests/cluster_integration.rs`) drive the whole `Cluster` end-to-end: boot, tick, host failure + evacuation, recovery, load injection.
- **Property-based tests** (`proptest`) assert capacity invariants hold after arbitrary sequences of ticks/bursts/failures/recoveries.
- **Benchmarks** (`criterion`) measure scheduling throughput as host count scales from 16 to 256, and batch-placement latency for autoscale bursts of 50–500 VMs.

CI (`.github/workflows/ci.yml`) runs all of the above plus a Next.js typecheck/build on every push.

### Keeping the deployed wasm artifact honest

`web/public/pkg` is a **committed build artifact** — Vercel deploys it as-is and never runs `wasm-pack` itself, so a stale binary there would silently ship an outdated simulation to production with no build failure anywhere. Since the compiled `.wasm` isn't bit-for-bit reproducible across builds even from identical source, CI can't just diff the binary; instead `scripts/wasm-pkg.sh` hashes the source inputs (`crates/engine/src`, `crates/wasm/src`, their `Cargo.toml`s, and the workspace lockfile) and records that hash in `web/public/pkg/.source-hash`. CI's `wasm` job recomputes the hash from current source and fails the build if it doesn't match — so forgetting to rebuild after a source change is caught automatically instead of silently drifting into production.

## License

MIT — see [LICENSE](LICENSE).
