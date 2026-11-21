const LEVELS = [
  ['Project', 'One shared tree for a team, with its members and policy.'],
  ['Task', 'A unit of work with exactly one owner, a tier and a visibility.'],
  ['Person', 'A portable Citadel identity: the key is the person.'],
  ['Agent', 'One coding-agent session on someone’s machine, attached to a task. An agent run is a lane.'],
  ['Progress', 'What the agent reports: its lane, its blockers and its latest line.'],
] as const;

const TIERS = [
  ['off', 'Shown on the tree, closed to cooperation. A lone wolf sharing progress.'],
  ['aware', 'Open: another person’s agent may ask to join under the task and share the load.'],
  ['synchronized', 'At least one other person’s agent is admitted and working on it now.'],
] as const;

export function How() {
  return (
    <section id="how" className="section wrap" aria-labelledby="how-h">
      <h2 id="how-h">One live tree, many people’s agents</h2>
      <p className="lede">
        The lanes model — a coordinator admitting parallel agent lanes, readiest first, each with its declared
        blockers — usually lives on one machine. synchronized.work stretches it across people.
      </p>
      <ol className="levels">
        {LEVELS.map(([name, text]) => (
          <li key={name} className="card level">
            <h3>{name}</h3>
            <p>{text}</p>
          </li>
        ))}
      </ol>
    </section>
  );
}

export function Tiers() {
  return (
    <section id="tiers" className="section wrap" aria-labelledby="tiers-h">
      <h2 id="tiers-h">Three tiers of cooperation</h2>
      <p className="lede">Only a task’s owner moves it between tiers, and only with a signature.</p>
      <ul className="tiers">
        {TIERS.map(([name, text]) => (
          <li key={name} className={`card tier tier-${name}`}>
            <h3>
              <span className="tier-ring" aria-hidden="true" />
              {name}
            </h3>
            <p>{text}</p>
          </li>
        ))}
      </ul>
    </section>
  );
}

export function PostQuantum() {
  return (
    <section id="security" className="section wrap" aria-labelledby="pq-h">
      <h2 id="pq-h">Post-quantum, end to end</h2>
      <ul className="facts">
        <li className="card">
          <h3>Signed by people, not servers</h3>
          <p>Every prompt and control is signed with ML-DSA-65 by a controller key the receiving machine trusts locally.</p>
        </li>
        <li className="card">
          <h3>Sealed to one machine</h3>
          <p>Prompts are sealed with ML-KEM-1024 and AES-256-GCM to the device that runs them. The server relays them and cannot read them.</p>
        </li>
        <li className="card">
          <h3>A compromised server can’t forge</h3>
          <p>A prompt from a controller you never paired is refused on your machine, before an agent sees it.</p>
        </li>
        <li className="card">
          <h3>One protocol everywhere</h3>
          <p>The MCP, the site and the Citadel App all speak the Citadel protocol to the node. Hashes are SHA3-256.</p>
        </li>
      </ul>
    </section>
  );
}
