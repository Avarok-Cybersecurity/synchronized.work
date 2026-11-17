import { Handle, Position, type Node, type NodeProps } from '@xyflow/react';
import type { ViewNode } from './types';
import { KIND_NAMES } from './tree';

export type CardNode = Node<{ view: ViewNode }, 'sw'>;

// One point of the tree. State is text beside its ring or badge, never colour alone.
export function NodeCard({ data, selected }: NodeProps<CardNode>) {
  const v = data.view;
  return (
    <div className={`node-card kind-${v.kind}${v.state ? ` state-${v.state}` : ''}${selected ? ' is-selected' : ''}`}>
      {v.parent !== null && <Handle type="target" position={Position.Top} isConnectable={false} />}
      <span className="node-kind">{KIND_NAMES[v.kind]}</span>
      <span className="node-label">{v.label}</span>
      <span className="node-detail">
        {v.state && <span className="node-state">{v.state}</span>}
        {v.detail}
      </span>
      <Handle type="source" position={Position.Bottom} isConnectable={false} />
    </div>
  );
}
