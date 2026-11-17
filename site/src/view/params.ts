// The view's sizes and gaps: one file, read by the site and by the Rust golden-layout test
// (crates/sw-view-wasm/tests/view.rs), so the tested layout is the shipped one.
import raw from './params.json';
import type { Kind, ViewParams } from './types';

function pair(kind: Kind, v: readonly number[]): [number, number] {
  const [w, h] = v;
  if (v.length !== 2 || w === undefined || h === undefined) throw new Error(`params.json: ${kind} must be [w, h]`);
  return [w, h];
}

export const VIEW_PARAMS: ViewParams = {
  node_gap: raw.node_gap,
  level_gap: raw.level_gap,
  sizes: {
    project: pair('project', raw.sizes.project),
    task: pair('task', raw.sizes.task),
    person: pair('person', raw.sizes.person),
    agent: pair('agent', raw.sizes.agent),
    progress: pair('progress', raw.sizes.progress),
  },
};
