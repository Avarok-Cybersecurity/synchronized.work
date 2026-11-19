// Path routing over the History API. The Worker serves the app shell for every unknown path
// that does not name a file (crates/sw-worker/src/fallback.rs), so deep links load directly.
import { useEffect, useState } from 'react';

export type Route = { page: 'landing' } | { page: 'demo' } | { page: 'auth-return' } | { page: 'not-found' };

export const DEMO_PATH = '/p/demo';

export function routeOf(path: string): Route {
  if (path === '/') return { page: 'landing' };
  if (path === DEMO_PATH) return { page: 'demo' };
  if (path === '/auth/return') return { page: 'auth-return' };
  return { page: 'not-found' };
}

export function navigate(path: string): void {
  history.pushState(null, '', path);
  dispatchEvent(new PopStateEvent('popstate'));
}

export function useRoute(): Route {
  const [path, setPath] = useState(location.pathname);
  useEffect(() => {
    const on = () => setPath(location.pathname);
    addEventListener('popstate', on);
    return () => removeEventListener('popstate', on);
  }, []);
  return routeOf(path);
}
