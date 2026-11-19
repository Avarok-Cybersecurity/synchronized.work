// The landing's animated background: a tree that grows edge by edge, carries a pulse from its
// root outward (two agents synchronizing), then dissolves and regrows from a new seed. Our own
// technique in the spirit of thomaspbraun.com / auth.metrale.ai; no code or assets copied.

export interface Palette {
  ink: string;
  primary: string;
  secondary: string;
}

interface Edge {
  ax: number;
  ay: number;
  bx: number;
  by: number;
  /** Seconds into the cycle when the edge starts drawing; also its depth order. */
  t0: number;
  depth: number;
}

export const PERIOD_S = 18;
const GROW_S = 9;
const PULSE_S = 5;
const DRAW_S = 0.9;
const POINTS = 46;

/** mulberry32: a small seeded generator, so a cycle is reproducible from its seed. */
function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Points in the unit square, each joined to its nearest earlier point: an organic tree. */
export function grow(seed: number): Edge[] {
  const r = rng(seed);
  const pts: { x: number; y: number; depth: number }[] = [{ x: 0.15 + r() * 0.7, y: 0.1 + r() * 0.2, depth: 0 }];
  const edges: Edge[] = [];
  for (let i = 1; i < POINTS; i++) {
    const p = { x: r(), y: r(), depth: 0 };
    let best = 0;
    let bestD = Infinity;
    pts.forEach((q, j) => {
      const d = (q.x - p.x) ** 2 + (q.y - p.y) ** 2;
      if (d < bestD) [best, bestD] = [j, d];
    });
    const parent = pts[best];
    if (!parent) continue;
    p.depth = parent.depth + 1;
    pts.push(p);
    edges.push({ ax: parent.x, ay: parent.y, bx: p.x, by: p.y, t0: (i / POINTS) * (GROW_S - DRAW_S), depth: p.depth });
  }
  return edges;
}

const clamp = (v: number) => Math.min(1, Math.max(0, v));

/** Draws the cycle at `t` seconds. `still` draws the grown tree with no pulse. */
export function draw(ctx: CanvasRenderingContext2D, w: number, h: number, edges: Edge[], t: number, pal: Palette, still: boolean): void {
  ctx.clearRect(0, 0, w, h);
  const fade = still ? 1 : 1 - clamp((t - GROW_S - PULSE_S) / (PERIOD_S - GROW_S - PULSE_S));
  const maxDepth = edges.reduce((m, e) => Math.max(m, e.depth), 1);
  const pulseAt = still ? -1 : ((t - GROW_S) / PULSE_S) * maxDepth;
  ctx.lineCap = 'round';
  for (const e of edges) {
    const p = still ? 1 : clamp((t - e.t0) / DRAW_S);
    if (p <= 0) continue;
    const ax = e.ax * w, ay = e.ay * h;
    const bx = ax + (e.bx * w - ax) * p, by = ay + (e.by * h - ay) * p;
    const lit = pulseAt >= 0 && Math.abs(pulseAt - e.depth) < 0.8;
    ctx.globalAlpha = 0.32 * fade + (lit ? 0.4 * fade : 0);
    ctx.strokeStyle = lit ? pal.secondary : pal.primary;
    ctx.lineWidth = lit ? 2.4 : 1.4;
    ctx.beginPath();
    ctx.moveTo(ax, ay);
    ctx.lineTo(bx, by);
    ctx.stroke();
    ctx.fillStyle = lit ? pal.secondary : p >= 1 ? pal.primary : pal.ink;
    ctx.globalAlpha = 0.45 * fade;
    ctx.beginPath();
    ctx.arc(bx, by, lit ? 4 : 2.6, 0, Math.PI * 2);
    ctx.fill();
  }
  ctx.globalAlpha = 1;
}
