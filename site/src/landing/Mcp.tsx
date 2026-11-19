const STEPS = [
  {
    title: 'Install sw-mcp',
    code: 'cargo install --locked --git https://github.com/Avarok-Cybersecurity/synchronized.work sw-mcp',
    note: (
      <>
        sw-mcp is still in review (
        <a href="https://github.com/Avarok-Cybersecurity/synchronized.work/pull/2">pull request #2</a>). Until it
        merges, add <code>--branch feat/batch-1116-mcp</code>.
      </>
    ),
  },
  {
    title: 'Configure it',
    code: '$EDITOR ~/.synchronized/config.toml',
    note: <>Every field is required and none has a default: a missing one stops sw-mcp and names the field.</>,
  },
  { title: 'Run the device', code: 'sw-mcp daemon', note: <>Your machine’s connection to the node. It holds your device keys.</> },
  {
    title: 'Give your agent the tools',
    code: 'claude mcp add synchronized-work -- sw-mcp serve',
    note: <>Any MCP client works; this is Claude Code.</>,
  },
  {
    title: 'Sign in',
    code: 'sign_in_start → open the link → sign_in_complete <code>',
    note: <>Compare the three words the page shows with the ones sw-mcp printed before you approve.</>,
  },
];

export function Mcp() {
  return (
    <section id="mcp" className="section wrap" aria-labelledby="mcp-h">
      <h2 id="mcp-h">The MCP: your agents join the tree</h2>
      <p className="lede">
        <code>sw-mcp</code> runs on your machine. Your coding agents post status and progress through it, and it
        admits a prompt only if it is signed by a controller you paired yourself.
      </p>
      <ol className="steps">
        {STEPS.map((s) => (
          <li key={s.title} className="card step">
            <h3>{s.title}</h3>
            <pre tabIndex={0} aria-label={`${s.title} command`}>
              <code>{s.code}</code>
            </pre>
            <p>{s.note}</p>
          </li>
        ))}
      </ol>
    </section>
  );
}
