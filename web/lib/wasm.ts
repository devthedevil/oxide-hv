"use client";

import type { ClusterSnapshot } from "./types";

export interface SimHandle {
  tick(): void;
  snapshot(): ClusterSnapshot;
  fail_host(host_id: number): void;
  recover_host(host_id: number): void;
  inject_burst(count: number): void;
  free(): void;
}

interface OxideWasmModule {
  default: () => Promise<unknown>;
  SimHandle: new () => SimHandle;
}

let modulePromise: Promise<OxideWasmModule> | null = null;

/**
 * The wasm-pack `--target web` glue expects to be loaded as a native ES
 * module so its internal `new URL('oxide_wasm_bg.wasm', import.meta.url)`
 * resolves against its own location under /pkg. `webpackIgnore` stops
 * webpack from trying to statically bundle it, leaving that resolution to
 * the browser at runtime.
 */
// A non-literal specifier keeps TypeScript from trying to statically resolve
// this as a project module — it's a static asset served from /public/pkg,
// only resolvable by the browser at runtime.
const PKG_ENTRY: string = "/pkg/oxide_wasm.js";

async function loadModule(): Promise<OxideWasmModule> {
  if (!modulePromise) {
    modulePromise = (async () => {
      const wasm = (await import(/* webpackIgnore: true */ PKG_ENTRY)) as OxideWasmModule;
      await wasm.default();
      return wasm;
    })();
  }
  return modulePromise;
}

export async function createSimHandle(): Promise<SimHandle> {
  const wasm = await loadModule();
  return new wasm.SimHandle();
}
