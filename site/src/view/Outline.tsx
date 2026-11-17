import { useMemo, useRef, useState, type KeyboardEvent } from 'react';
import type { ViewGraph } from './types';
import { describe, indexGraph, visibleIds } from './tree';

// The tree as an ARIA tree (design §8.3): the keyboard and screen-reader way through the same
// graph, sharing one selection with the canvas. Arrow keys move, Left/Right collapse/expand.
export function Outline({ graph, selected, onSelect }: { graph: ViewGraph; selected: string | null; onSelect: (id: string) => void }) {
  const index = useMemo(() => indexGraph(graph), [graph]);
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set());
  const ids = useMemo(() => visibleIds(index, collapsed), [index, collapsed]);
  const refs = useRef(new Map<string, HTMLLIElement>());
  const current = selected !== null && ids.includes(selected) ? selected : (ids[0] ?? null);

  const focus = (id: string | undefined) => {
    if (id === undefined) return;
    onSelect(id);
    refs.current.get(id)?.focus();
  };
  const toggle = (id: string, open: boolean) => {
    const next = new Set(collapsed);
    if (open) next.delete(id);
    else next.add(id);
    setCollapsed(next);
  };
  const onKey = (e: KeyboardEvent<HTMLUListElement>) => {
    if (current === null) return;
    const at = ids.indexOf(current);
    const node = index.byId.get(current);
    const hasKids = (index.children.get(current) ?? []).length > 0;
    const moves: Record<string, () => void> = {
      ArrowDown: () => focus(ids[at + 1]),
      ArrowUp: () => focus(ids[at - 1]),
      Home: () => focus(ids[0]),
      End: () => focus(ids[ids.length - 1]),
      ArrowRight: () => (hasKids && collapsed.has(current) ? toggle(current, true) : focus(index.children.get(current)?.[0]?.id)),
      ArrowLeft: () => (hasKids && !collapsed.has(current) ? toggle(current, false) : focus(node?.parent ?? undefined)),
    };
    const move = moves[e.key];
    if (move) {
      e.preventDefault();
      move();
    }
  };

  const render = (parent: string | null) => (
    <>
      {(index.children.get(parent) ?? []).map((n) => {
        const kids = (index.children.get(n.id) ?? []).length > 0;
        const open = !collapsed.has(n.id);
        return (
          <li
            key={n.id}
            ref={(el) => {
              if (el) refs.current.set(n.id, el);
              else refs.current.delete(n.id);
            }}
            role="treeitem"
            aria-level={index.depth.get(n.id)}
            aria-selected={n.id === current}
            aria-expanded={kids ? open : undefined}
            aria-label={describe(n)}
            tabIndex={n.id === current ? 0 : -1}
            onClick={(e) => {
              e.stopPropagation();
              focus(n.id);
            }}
            className={`outline-item kind-${n.kind}`}
          >
            <span className="outline-row" aria-hidden="true">
              <span className="outline-kind">{n.kind}</span> {n.label}
              {n.state && <span className="node-state">{n.state}</span>}
            </span>
            {kids && open && <ul role="group">{render(n.id)}</ul>}
          </li>
        );
      })}
    </>
  );

  return (
    <ul role="tree" aria-label="Project tree" className="outline" onKeyDown={onKey}>
      {render(null)}
    </ul>
  );
}
