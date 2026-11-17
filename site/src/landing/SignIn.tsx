import { useCallback, useEffect, useState } from 'react';
import { fetchHealth, type NodeHealth } from '../health';
import { startSignIn, SignInUnavailable } from '../signin';
import { Link } from '../Chrome';
import { DEMO_PATH } from '../router';

type Shown = NodeHealth | 'checking';

// The one sign-in entry. It asks /healthz first and offers sign-in only when the node says it
// is ready; every other answer is said plainly, and nothing here pretends to be signed in.
export function SignIn() {
  const [health, setHealth] = useState<Shown>('checking');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const check = useCallback(() => {
    setHealth('checking');
    fetchHealth().then(setHealth, () => setHealth('unreachable'));
  }, []);
  useEffect(check, [check]);

  const go = async () => {
    setBusy(true);
    setError(null);
    try {
      location.assign(await startSignIn());
    } catch (e) {
      setError(e instanceof SignInUnavailable ? e.message : 'Sign-in could not start in this browser.');
      setBusy(false);
    }
  };

  return (
    <div className="sign-in card" data-health={health}>
      <h3>Sign in with Citadel</h3>
      <div role="status" aria-live="polite" className="sign-in-status">
        {health === 'checking' && <p>Checking whether sign-in is open…</p>}
        {health === 'unprovisioned' && (
          <p>
            <strong>Sign-in opens soon.</strong> The synchronized.work node is deployed but not yet
            provisioned with its keys, so nobody can sign in today.
          </p>
        )}
        {health === 'unreachable' && (
          <p>
            <strong>We couldn't reach the node,</strong> so we can't say whether sign-in is open.
          </p>
        )}
        {health === 'ready' && <p>You sign in at work.avarok.net with the Citadel Agent, then come back here.</p>}
        {error && <p className="error">{error}</p>}
      </div>
      <div className="actions">
        {health === 'ready' && (
          <button type="button" className="btn btn-primary" onClick={go} disabled={busy}>
            Sign in with Citadel
          </button>
        )}
        {health === 'unreachable' && (
          <button type="button" className="btn btn-primary" onClick={check}>
            Check again
          </button>
        )}
        {health !== 'ready' && (
          <Link to={DEMO_PATH} className="btn btn-ghost">
            Explore the demo
          </Link>
        )}
      </div>
    </div>
  );
}
