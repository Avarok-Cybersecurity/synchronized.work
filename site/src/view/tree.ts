// Parent/child lookups over the view graph, for the outline and the detail pane.
import type { ViewGraph, ViewNode } from './types';

export interface Indexed {
  byId: Map<string, ViewNode>;
  children: Map<string | null, ViewNode[]>;
  depth: Map<string, number>;
}

export function indexGraph(graph: ViewGraph): Indexed {
  const byId = new Map<string, ViewNode>();
  const children = new Map<string | null, ViewNode[]>();
  const depth = new Map<string, number>();
  for (const node of graph.nodes) {
    byId.set(node.id, node);
    const siblings = children.get(node.parent) ?? [];
    siblings.push(node);
    children.set(node.parent, siblings);
    depth.set(node.id, node.parent === null ? 1 : (depth.get(node.parent) ?? 0) + 1);
  }
  return { byId, children, depth };
}

/** The ids shown in the outline, in order, skipping the descendants of collapsed items. */
export function visibleIds(index: Indexed, collapsed: ReadonlySet<string>): string[] {
  const out: string[] = [];
  const walk = (parent: string | null) => {
    for (const child of index.children.get(parent) ?? []) {
      out.push(child.id);
      if (!collapsed.has(child.id)) walk(child.id);
    }
  };
  walk(null);
  return out;
}

export const KIND_NAMES = {
  project: 'Project',
  task: 'Task',
  person: 'Person',
  agent: 'Agent',
  progress: 'Progress',
} as const;

export function describe(node: ViewNode): string {
  const state = node.state ? `, ${node.kind === 'task' ? 'tier' : 'lane'} ${node.state}` : '';
  return `${KIND_NAMES[node.kind]}: ${node.label}${state}. ${node.detail}`;
}
