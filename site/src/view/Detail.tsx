import { useMemo } from 'react';
import type { ViewGraph } from './types';
import { KIND_NAMES, indexGraph } from './tree';

export function Detail({ graph, selected }: { graph: ViewGraph; selected: string | null }) {
  const index = useMemo(() => indexGraph(graph), [graph]);
  const node = selected === null ? undefined : index.byId.get(selected);
  const parent = node?.parent ? index.byId.get(node.parent) : undefined;
  return (
    <div className="detail card" aria-live="polite">
      <h2>Details</h2>
      {node === undefined ? (
        <p>Select a point in the tree to see it here.</p>
      ) : (
        <dl>
          <dt>{KIND_NAMES[node.kind]}</dt>
          <dd>{node.label}</dd>
          {node.state && (
            <>
              <dt>{node.kind === 'task' ? 'Tier' : 'Lane'}</dt>
              <dd>{node.state}</dd>
            </>
          )}
          <dt>About</dt>
          <dd>{node.detail}</dd>
          {parent && (
            <>
              <dt>Under</dt>
              <dd>{parent.label}</dd>
            </>
          )}
        </dl>
      )}
    </div>
  );
}
