import { useEffect } from 'react';
import { Link } from './Chrome';
import { DEMO_PATH } from './router';
import { forgetPendingSignIn } from './signin';

// The Worker answers the issuer's POST with a redirect here and drops the one-time code
// unread (crates/sw-worker/src/routes.rs). Finishing needs this browser's Controller key and
// the node session, which the site does not have yet; the page says so instead of pretending.
export function AuthReturn() {
  useEffect(forgetPendingSignIn, []);
  return (
    <section className="wrap page-pad narrow">
      <h1>Sign-in can't finish here yet</h1>
      <p>
        work.avarok.net sent you back, but this site cannot complete a sign-in today. Finishing it needs
        this browser's own controller key and a live session with the synchronized.work node, and both
        arrive in a later release.
      </p>
      <p>Nothing was stored. The one-time code was discarded, and it is useless without the secret that never left this tab.</p>
      <p className="actions">
        <Link to="/" className="btn btn-primary">
          Back to the start
        </Link>
        <Link to={DEMO_PATH} className="btn btn-ghost">
          Explore the demo
        </Link>
      </p>
    </section>
  );
}
