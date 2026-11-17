// What the sign-in entry may show, from the Worker's /healthz
// (crates/sw-worker/src/routes.rs): `{"v":1,"sha":…,"node":"ready"|"unprovisioned"}`, 200 only
// when the node can start. Anything else is "unreachable": never presented as open.

export type NodeHealth = 'ready' | 'unprovisioned' | 'unreachable';

export function healthOf(status: number, body: unknown): NodeHealth {
  if (typeof body !== 'object' || body === null) return 'unreachable';
  const { v, node } = body as { v?: unknown; node?: unknown };
  if (v !== 1) return 'unreachable';
  if (status === 200 && node === 'ready') return 'ready';
  if (status === 503 && node === 'unprovisioned') return 'unprovisioned';
  return 'unreachable';
}

export async function fetchHealth(): Promise<NodeHealth> {
  try {
    const response = await fetch('/healthz', { cache: 'no-store' });
    return healthOf(response.status, await response.json());
  } catch {
    return 'unreachable';
  }
}
