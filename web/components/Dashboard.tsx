"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { createSimHandle, type SimHandle } from "@/lib/wasm";
import type { ClusterSnapshot } from "@/lib/types";
import MetricsBar from "./MetricsBar";
import HostGrid from "./HostGrid";
import EventLog from "./EventLog";

const TICK_INTERVAL_MS = 300;

export default function Dashboard() {
  const handleRef = useRef<SimHandle | null>(null);
  const [snapshot, setSnapshot] = useState<ClusterSnapshot | null>(null);
  const [running, setRunning] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    createSimHandle()
      .then((handle) => {
        if (cancelled) {
          handle.free();
          return;
        }
        handleRef.current = handle;
        setSnapshot(handle.snapshot());
      })
      .catch((err) => setError(String(err)));

    return () => {
      cancelled = true;
      handleRef.current?.free();
      handleRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (!running) return;
    const id = setInterval(() => {
      const handle = handleRef.current;
      if (!handle) return;
      handle.tick();
      setSnapshot(handle.snapshot());
    }, TICK_INTERVAL_MS);
    return () => clearInterval(id);
  }, [running]);

  const failRandomHost = useCallback(() => {
    const handle = handleRef.current;
    if (!handle || !snapshot) return;
    const healthy = snapshot.hosts.filter((h) => h.healthy);
    if (healthy.length === 0) return;
    const target = healthy[Math.floor(Math.random() * healthy.length)];
    handle.fail_host(target.id);
    setSnapshot(handle.snapshot());
  }, [snapshot]);

  const failHost = useCallback((id: number) => {
    handleRef.current?.fail_host(id);
    if (handleRef.current) setSnapshot(handleRef.current.snapshot());
  }, []);

  const recoverHost = useCallback((id: number) => {
    handleRef.current?.recover_host(id);
    if (handleRef.current) setSnapshot(handleRef.current.snapshot());
  }, []);

  const injectBurst = useCallback(() => {
    handleRef.current?.inject_burst(25);
    if (handleRef.current) setSnapshot(handleRef.current.snapshot());
  }, []);

  if (error) {
    return <div className="p-8 text-crit">Failed to load simulation engine: {error}</div>;
  }

  if (!snapshot) {
    return <div className="p-8 text-gray-500">Booting hypervisor fleet…</div>;
  }

  return (
    <main className="mx-auto flex min-h-screen max-w-[1400px] flex-col gap-5 p-6">
      <header className="flex flex-col gap-1">
        <h1 className="text-2xl font-bold text-white">
          Oxide<span className="text-accent">HV</span>
        </h1>
        <p className="text-sm text-gray-500">
          A NUMA-aware hypervisor scheduler, live-migration engine, and elastic autoscaler — written in Rust, compiled to WebAssembly, running entirely
          client-side.
        </p>
      </header>

      <MetricsBar metrics={snapshot.metrics} />

      <div className="flex flex-wrap items-center gap-2">
        <button onClick={() => setRunning((r) => !r)} className="rounded border border-border px-3 py-1.5 text-xs hover:bg-panel">
          {running ? "Pause" : "Resume"}
        </button>
        <button onClick={injectBurst} className="rounded border border-accent/40 px-3 py-1.5 text-xs text-accent hover:bg-accent/10">
          Inject Load Burst (+25 VMs)
        </button>
        <button onClick={failRandomHost} className="rounded border border-crit/40 px-3 py-1.5 text-xs text-crit hover:bg-crit/10">
          Fail Random Host
        </button>
      </div>

      <div className="grid flex-1 grid-cols-1 gap-4 lg:grid-cols-[3fr_1fr]">
        <HostGrid hosts={snapshot.hosts} vms={snapshot.vms} onFail={failHost} onRecover={recoverHost} />
        <div className="min-h-[300px] lg:min-h-0">
          <EventLog events={snapshot.events} />
        </div>
      </div>
    </main>
  );
}
