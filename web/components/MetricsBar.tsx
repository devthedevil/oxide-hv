import type { ClusterMetrics } from "@/lib/types";

function pct(v: number): string {
  return `${(v * 100).toFixed(0)}%`;
}

function Stat({ label, value, tone }: { label: string; value: string; tone?: "accent" | "warn" | "crit" }) {
  const toneClass = tone === "accent" ? "text-accent" : tone === "warn" ? "text-warn" : tone === "crit" ? "text-crit" : "text-white";
  return (
    <div className="flex flex-col gap-1 rounded-lg border border-border bg-panel px-4 py-3 min-w-[128px]">
      <span className="text-[11px] uppercase tracking-wide text-gray-500">{label}</span>
      <span className={`text-xl font-semibold ${toneClass}`}>{value}</span>
    </div>
  );
}

export default function MetricsBar({ metrics }: { metrics: ClusterMetrics }) {
  return (
    <div className="flex flex-wrap gap-3">
      <Stat label="Tick" value={metrics.tick.toString()} />
      <Stat label="Hosts" value={`${metrics.healthy_hosts}/${metrics.total_hosts}`} tone={metrics.healthy_hosts < metrics.total_hosts ? "crit" : undefined} />
      <Stat label="VMs Running" value={metrics.running_vms.toString()} tone="accent" />
      <Stat label="VMs Migrating" value={metrics.migrating_vms.toString()} tone={metrics.migrating_vms > 0 ? "warn" : undefined} />
      <Stat label="Avg CPU" value={pct(metrics.avg_cpu_utilization)} tone={metrics.avg_cpu_utilization > 0.8 ? "crit" : undefined} />
      <Stat label="Avg Mem" value={pct(metrics.avg_mem_utilization)} />
      <Stat label="Avg NVMe" value={pct(metrics.avg_nvme_utilization)} />
      <Stat label="GPU Util" value={pct(metrics.gpu_utilization)} />
      <Stat label="Migrations" value={metrics.migrations_total.toString()} />
      <Stat label="Scale Out / In" value={`${metrics.scale_out_events_total} / ${metrics.scale_in_events_total}`} />
      <Stat label="Placement Fails" value={metrics.placement_failures_total.toString()} tone={metrics.placement_failures_total > 0 ? "warn" : undefined} />
    </div>
  );
}
