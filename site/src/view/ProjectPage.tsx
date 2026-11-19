import { useEffect, useState } from 'react';
import { ReactFlowProvider } from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import '../styles/view.css';
import { demoView } from '../wasm/core';
import type { ViewGraph } from './types';
import { VIEW_PARAMS } from './params';
import { Flow } from './Flow';
import { Outline } from './Outline';
import { Detail } from './Detail';

type Loaded = { graph: ViewGraph } | { error: string } | null;

// Without a session the only project is the demo, folded from the protocol fixtures by the
// same Rust viewer logic a live project uses. It says "demo" before anything else.
export function ProjectPage() {
  const [loaded, setLoaded] = useState<Loaded>(null);
  const [selected, setSelected] = useState<string | null>(null);
  useEffect(() => {
    demoView(VIEW_PARAMS).then(
      (graph) => setLoaded({ graph }),
      (e: unknown) => setLoaded({ error: e instanceof Error ? e.message : String(e) }),
    );
  }, []);
  return (
    <section className="project wrap-wide" aria-labelledby="project-h">
      <div className="demo-banner" role="note">
        <strong>Demo project.</strong> Sample data built from the protocol’s test fixtures, not a live project.
        Live projects appear here after you sign in, once sign-in opens.
      </div>
      <h1 id="project-h">Demo project</h1>
      {loaded === null && <p role="status">Laying out the tree…</p>}
      {loaded !== null && 'error' in loaded && (
        <p role="alert" className="error">
          The view core failed to load: {loaded.error}
        </p>
      )}
      {loaded !== null && 'graph' in loaded && (
        <div className="project-grid">
          <div className="flow-frame">
            <ReactFlowProvider>
              <Flow graph={loaded.graph} selected={selected} onSelect={setSelected} />
            </ReactFlowProvider>
          </div>
          <aside className="project-side" aria-label="Tree and details">
            <h2>Tree</h2>
            <Outline graph={loaded.graph} selected={selected} onSelect={setSelected} />
            <Detail graph={loaded.graph} selected={selected} />
          </aside>
        </div>
      )}
    </section>
  );
}
