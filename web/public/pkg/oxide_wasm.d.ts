/* tslint:disable */
/* eslint-disable */

/**
 * Opaque handle the JS/TS dashboard drives one `tick()` at a time,
 * pulling a JSON snapshot after each step to render.
 */
export class SimHandle {
    free(): void;
    [Symbol.dispose](): void;
    fail_host(host_id: number): void;
    inject_burst(count: number): void;
    constructor();
    recover_host(host_id: number): void;
    snapshot(): any;
    tick(): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_simhandle_free: (a: number, b: number) => void;
    readonly simhandle_fail_host: (a: number, b: number) => void;
    readonly simhandle_inject_burst: (a: number, b: number) => void;
    readonly simhandle_new: () => number;
    readonly simhandle_recover_host: (a: number, b: number) => void;
    readonly simhandle_snapshot: (a: number) => [number, number, number];
    readonly simhandle_tick: (a: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
