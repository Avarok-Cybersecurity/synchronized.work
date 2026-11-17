import { Suspense, lazy, useEffect, useRef } from 'react';
import { useRoute } from './router';
import { Header, Footer } from './Chrome';
import { Landing } from './landing/Landing';
import { AuthReturn } from './AuthReturn';
import { Background } from './background/Background';

// The graph view (React Flow and the wasm core) loads only when a project is opened.
const ProjectPage = lazy(() => import('./view/ProjectPage').then((m) => ({ default: m.ProjectPage })));

const TITLES = {
  landing: 'synchronized.work — agents that coordinate across people',
  demo: 'Demo project — synchronized.work',
  'auth-return': 'Signing in — synchronized.work',
  'not-found': 'Not found — synchronized.work',
} as const;

export function App() {
  const route = useRoute();
  const main = useRef<HTMLElement>(null);
  const first = useRef(true);
  useEffect(() => {
    document.title = TITLES[route.page];
    // Move focus to the new page's content on navigation (never on first load).
    if (first.current) first.current = false;
    else main.current?.focus();
  }, [route.page]);
  return (
    <>
      <a className="skip" href="#main">Skip to content</a>
      {route.page === 'landing' && <Background />}
      <Header />
      <main id="main" ref={main} tabIndex={-1}>
        {route.page === 'landing' && <Landing />}
        {route.page === 'demo' && (
          <Suspense fallback={<p className="wrap page-pad" role="status">Loading the project view…</p>}>
            <ProjectPage />
          </Suspense>
        )}
        {route.page === 'auth-return' && <AuthReturn />}
        {route.page === 'not-found' && (
          <section className="wrap page-pad">
            <h1>Not found</h1>
            <p>There is no page at this address.</p>
          </section>
        )}
      </main>
      <Footer />
    </>
  );
}
