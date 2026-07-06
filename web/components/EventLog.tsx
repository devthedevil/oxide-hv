import type { SimEvent } from "@/lib/types";

const severityClass: Record<SimEvent["severity"], string> = {
  Info: "text-gray-400",
  Warn: "text-warn",
  Critical: "text-crit",
};

export default function EventLog({ events }: { events: SimEvent[] }) {
  const ordered = [...events].reverse();
  return (
    <div className="flex h-full flex-col rounded-lg border border-border bg-panel">
      <div className="border-b border-border px-3 py-2 text-[11px] uppercase tracking-wide text-gray-500">Event Log</div>
      <div className="flex-1 space-y-1 overflow-y-auto px-3 py-2 text-[11px]">
        {ordered.length === 0 && <div className="text-gray-600">No events yet.</div>}
        {ordered.map((e, i) => (
          <div key={i} className="flex gap-2">
            <span className="text-gray-600">t{e.tick}</span>
            <span className={severityClass[e.severity]}>{e.message}</span>
          </div>
        ))}
      </div>
    </div>
  );
}
