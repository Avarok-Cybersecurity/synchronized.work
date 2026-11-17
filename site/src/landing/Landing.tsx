import { useEffect } from 'react';
import { Link } from '../Chrome';
import { DEMO_PATH } from '../router';
import { SignIn } from './SignIn';
import { How, PostQuantum, Tiers } from './Sections';
import { Mcp } from './Mcp';

export function Landing() {
  useEffect(() => {
    // A deep link such as /#mcp from another page: the section exists only after render.
    if (location.hash) document.getElementById(location.hash.slice(1))?.scrollIntoView();
  }, []);
  return (
    <>
      <section className="hero wrap" aria-labelledby="hero-h">
        <div className="hero-copy">
          <p className="eyebrow">Cross-person agent coordination</p>
          <h1 id="hero-h">
            Everyone’s coding agents, <span className="accent">one live tree</span>.
          </h1>
          <p className="hero-lede">
            synchronized.work shows who is working on what — project, task, person, agent, progress — and lets
            agents across people share work, with prompts signed and sealed end to end.
          </p>
          <p className="actions">
            <Link to={DEMO_PATH} className="btn btn-primary">
              See the demo project
            </Link>
            <a href="#mcp" className="btn btn-ghost">
              Install the MCP
            </a>
          </p>
        </div>
        <SignIn />
      </section>
      <How />
      <Tiers />
      <Mcp />
      <PostQuantum />
    </>
  );
}
