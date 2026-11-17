import { useEffect, useMemo } from 'react';
import { ReactFlow, Controls, useReactFlow, type Edge } from '@xyflow/react';
import type { ViewGraph } from './types';
import { NodeCard, type CardNode } from './NodeCard';
import { describe } from './tree';

const NODE_TYPES = { sw: NodeCard };
const reducedMotion = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

// React Flow draws exactly the positions the Rust core computed (centres → top-left corners).
// It is view only: nothing can be dragged, connected or deleted here.
export function Flow({ graph, selected, onSelect }: { graph: ViewGraph; selected: string | null; onSelect: (id: string) => void }) {
  const { setCenter, getZoom } = useReactFlow();
  const nodes: CardNode[] = useMemo(
    () =>
      graph.nodes.map((v) => ({
        id: v.id,
        type: 'sw',
        position: { x: v.x - v.w / 2, y: v.y - v.h / 2 },
        width: v.w,
        height: v.h,
        data: { view: v },
        selected: v.id === selected,
        ariaLabel: describe(v),
        draggable: false,
        connectable: false,
        deletable: false,
      })),
    [graph, selected],
  );
  const edges: Edge[] = useMemo(
    () =>
      graph.nodes.flatMap((v) => (v.parent === null ? [] : [{ id: `${v.parent}->${v.id}`, source: v.parent, target: v.id, type: 'smoothstep', focusable: false }])),
    [graph],
  );
  useEffect(() => {
    const v = selected === null ? undefined : graph.nodes.find((n) => n.id === selected);
    if (v) void setCenter(v.x, v.y, { zoom: Math.max(getZoom(), 0.8), duration: reducedMotion() ? 0 : 300 });
  }, [selected, graph, setCenter, getZoom]);
  return (
    <ReactFlow
      nodes={nodes}
      edges={edges}
      nodeTypes={NODE_TYPES}
      onNodeClick={(_, n) => onSelect(n.id)}
      nodesDraggable={false}
      nodesConnectable={false}
      edgesFocusable={false}
      deleteKeyCode={null}
      fitView
      minZoom={0.2}
      maxZoom={1.5}
      aria-label="Project tree, drawn as a graph"
    >
      <Controls showInteractive={false} />
    </ReactFlow>
  );
}
