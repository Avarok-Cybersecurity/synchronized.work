// The view graph the Rust core returns (crates/sw-view-wasm/src/view.rs, serde names). It is
// checked here, at the boundary, so the rest of the view never handles `unknown`.

export type Kind = 'project' | 'task' | 'person' | 'agent' | 'progress';
const KINDS: readonly Kind[] = ['project', 'task', 'person', 'agent', 'progress'];

export interface ViewParams {
  node_gap: number;
  level_gap: number;
  sizes: Record<Kind, [number, number]>;
}

export interface ViewNode {
  id: string;
  parent: string | null;
  kind: Kind;
  label: string;
  detail: string;
  state: string | null;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface ViewGraph {
  demo: boolean;
  nodes: ViewNode[];
}

function fail(what: string): never {
  throw new Error(`view core returned a malformed graph: ${what}`);
}

const isRecord = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null;
const str = (v: unknown, what: string): string => (typeof v === 'string' ? v : fail(what));
const num = (v: unknown, what: string): number => (typeof v === 'number' && Number.isFinite(v) ? v : fail(what));
const strOrNull = (v: unknown, what: string): string | null => (v === null ? null : str(v, what));

function parseNode(v: unknown): ViewNode {
  if (!isRecord(v)) fail('node');
  const kind = str(v.kind, 'kind');
  if (!KINDS.includes(kind as Kind)) fail(`kind ${kind}`);
  return {
    id: str(v.id, 'id'),
    parent: strOrNull(v.parent, 'parent'),
    kind: kind as Kind,
    label: str(v.label, 'label'),
    detail: str(v.detail, 'detail'),
    state: strOrNull(v.state, 'state'),
    x: num(v.x, 'x'),
    y: num(v.y, 'y'),
    w: num(v.w, 'w'),
    h: num(v.h, 'h'),
  };
}

export function parseViewGraph(v: unknown): ViewGraph {
  if (!isRecord(v) || typeof v.demo !== 'boolean' || !Array.isArray(v.nodes)) fail('graph');
  return { demo: v.demo, nodes: v.nodes.map(parseNode) };
}
