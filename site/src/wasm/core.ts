// The Rust core (crates/sw-view-wasm), loaded once. Every call returns parsed, checked data
// or throws: nothing here falls back to a value the core did not compute.
import init, { demo_view, sign_in_link } from './pkg/sw_view_wasm.js';
import wasmUrl from './pkg/sw_view_wasm_bg.wasm?url';
import { parseViewGraph, type ViewGraph, type ViewParams } from '../view/types';

let loading: Promise<void> | null = null;

export function loadCore(): Promise<void> {
  loading ??= init({ module_or_path: wasmUrl }).then(() => undefined);
  return loading;
}

export async function demoView(params: ViewParams): Promise<ViewGraph> {
  await loadCore();
  return parseViewGraph(JSON.parse(demo_view(JSON.stringify(params))));
}

export async function signInLink(verifier: Uint8Array, state: Uint8Array): Promise<string> {
  await loadCore();
  return sign_in_link(verifier, state);
}
