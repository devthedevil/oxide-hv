import type { Host, Vm } from "@/lib/types";

function UtilBar({ label, ratio }: { label: string; ratio: number }) {
  const pct = Math.min(100, ratio * 100);
  const color = pct > 85 ? "bg-crit" : pct > 60 ? "bg-warn" : "bg-accent";
  return (
    <div className="flex items-center gap-2 text-[11px]">
      <span className="w-9 text-gray-500">{label}</span>
      <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-[#1a212c]">
        <div className={`h-full rounded-full ${color}`} style={{ width: `${pct}%` }} />
      </div>
      <span className="w-9 text-right text-gray-400">{pct.toFixed(0)}%</span>
    </div>
  );
}

function cpuUtil(host: Host): number {
  const cap = host.numa_nodes.reduce((s, n) => s + n.cpu_capacity_millicores, 0);
  const used = host.numa_nodes.reduce((s, n) => s + n.cpu_used_millicores, 0);
  return cap === 0 ? 0 : used / cap;
}

function memUtil(host: Host): number {
  const cap = host.numa_nodes.reduce((s, n) => s + n.mem_capacity_mb, 0);
  const used = host.numa_nodes.reduce((s, n) => s + n.mem_used_mb, 0);
  return cap === 0 ? 0 : used / cap;
}

function nvmeUtil(host: Host): number {
  const cap = host.nvme_devices.reduce((s, d) => s + d.max_iops, 0);
  const used = host.nvme_devices.reduce((s, d) => s + d.used_iops, 0);
  return cap === 0 ? 0 : used / cap;
}

export default function HostGrid({ hosts, vms, onFail, onRecover }: { hosts: Host[]; vms: Vm[]; onFail: (id: number) => void; onRecover: (id: number) => void }) {
  const outboundMigrations = new Map<number, number>();
  for (const vm of vms) {
    if (vm.state.kind === "Migrating" && vm.host_id !== undefined) {
      outboundMigrations.set(vm.host_id, (outboundMigrations.get(vm.host_id) ?? 0) + 1);
    }
  }

  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-6">
      {hosts.map((host) => {
        const migrating = outboundMigrations.get(host.id) ?? 0;
        const usedGpus = host.gpus.filter((g) => g.assigned_to !== null).length;
        return (
          <div key={host.id} className={`flex flex-col gap-2 rounded-lg border px-3 py-3 ${host.healthy ? "border-border bg-panel" : "border-crit/60 bg-crit/10"}`}>
            <div className="flex items-center justify-between">
              <span className="text-xs font-semibold">Host {host.id}</span>
              <span className={`h-2 w-2 rounded-full ${host.healthy ? "bg-accent" : "bg-crit"}`} title={host.healthy ? "healthy" : "failed"} />
            </div>

            <UtilBar label="CPU" ratio={cpuUtil(host)} />
            <UtilBar label="Mem" ratio={memUtil(host)} />
            {host.nvme_devices.length > 0 && <UtilBar label="NVMe" ratio={nvmeUtil(host)} />}

            <div className="flex items-center justify-between text-[11px] text-gray-400">
              <span>{host.vm_ids.length} VMs</span>
              <span>
                GPU {usedGpus}/{host.gpus.length}
              </span>
            </div>

            {migrating > 0 && <span className="rounded bg-warn/20 px-1.5 py-0.5 text-[10px] text-warn">{migrating} migrating out</span>}

            <div className="mt-1 flex gap-1.5">
              {host.healthy ? (
                <button onClick={() => onFail(host.id)} className="flex-1 rounded border border-crit/40 px-2 py-1 text-[10px] text-crit hover:bg-crit/10">
                  Fail
                </button>
              ) : (
                <button onClick={() => onRecover(host.id)} className="flex-1 rounded border border-accent/40 px-2 py-1 text-[10px] text-accent hover:bg-accent/10">
                  Recover
                </button>
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
