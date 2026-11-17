import { useState, type MouseEvent, type ReactNode } from 'react';
import { navigate, DEMO_PATH } from './router';
import { choose, effectiveTheme } from './theme';

export function Link({ to, children, className }: { to: string; children: ReactNode; className?: string }) {
  const onClick = (e: MouseEvent<HTMLAnchorElement>) => {
    if (e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0) return;
    e.preventDefault();
    navigate(to);
    scrollTo({ top: 0 });
  };
  return (
    <a href={to} onClick={onClick} className={className}>
      {children}
    </a>
  );
}

function ThemeToggle() {
  const [dark, setDark] = useState(() => effectiveTheme() === 'dark');
  const flip = () => {
    choose(dark ? 'light' : 'dark');
    setDark(!dark);
  };
  return (
    <button type="button" className="icon-btn" aria-pressed={dark} onClick={flip}>
      <span aria-hidden="true">{dark ? '☾' : '☀'}</span>
      <span className="sr-only">Dark theme</span>
    </button>
  );
}

export function Header() {
  return (
    <header className="site-header">
      <div className="wrap header-inner">
        <Link to="/" className="brand">
          <img src="/favicon.svg" alt="" width="28" height="28" />
          <span>synchronized.work</span>
        </Link>
        <nav aria-label="Main">
          <ul className="nav-links">
            <li>
              <a href="/#how">How it works</a>
            </li>
            <li>
              <a href="/#mcp">MCP</a>
            </li>
            <li>
              <Link to={DEMO_PATH}>Demo</Link>
            </li>
          </ul>
        </nav>
        <ThemeToggle />
      </div>
    </header>
  );
}

export function Footer() {
  return (
    <footer className="site-footer">
      <div className="wrap footer-inner">
        <p>
          synchronized.work is open source, by <a href="https://avarok.net">Avarok Cybersecurity</a>. No
          trackers, no analytics.
        </p>
        <ul className="footer-links">
          <li>
            <a href="https://github.com/Avarok-Cybersecurity/synchronized.work">Source on GitHub</a>
          </li>
          <li>
            <a href="https://avarok.net">avarok.net</a>
          </li>
          <li>
            <a href="https://github.com/tbraun96/tbraun96">Thomas Braun</a>
          </li>
        </ul>
      </div>
    </footer>
  );
}
