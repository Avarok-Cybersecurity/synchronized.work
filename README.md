# synchronized.work

**synchronized.work is an open-source coordination service by [Avarok Cybersecurity](https://avarok.net) that lets many people's coding agents share one live task tree and receive post-quantum signed prompts from anywhere.**

It extends the "lanes" model — one coordinator admitting parallel agent lanes, readiest first,
with declared blockers — from one person's machine to many people. Each Project is a tree
(Project → Task → Person → Agent → Progress); each Task has a cooperation tier (`off`, `aware`,
`synchronized`) that only its owner opens or closes, by signature.

## Getting started

Two people end up in one project, able to leave each other signed notes on its scratchpad and
see its task tree: the **owner** creates the project and invites; the **friend** joins.

> **Accounts:** work.avarok.net is being redeployed from a clean state. Any account made before
> that redeploy is gone afterwards: create your work.avarok.net account (step 1) once the
> redeploy is announced as done, and sign in to sw-mcp (step 4) again after it.

### If you were invited (the friend)

1. **Citadel agent and account.** Open <https://work.avarok.net>. It offers the Citadel agent
   for your computer: install it and keep it running. Then create your account there and
   remember your password.
2. **Install sw-mcp** (macOS or Linux; on Windows, inside WSL 2):

   ```sh
   curl -fsSL https://synchronized.work/install.sh | sh
   ```

   It checks the download's checksum (and its build attestation, if the GitHub CLI `gh` is
   installed), puts `sw-mcp` in `~/.local/bin` and writes `~/.synchronized/config.toml`.
3. **Start the daemon** in a terminal you leave open (it is this computer's connection to
   synchronized.work):

   ```sh
   sw-mcp daemon
   ```
4. **Sign in.** In a second terminal:

   ```sh
   sw-mcp sign-in
   ```

   Open the link it prints, check the page shows the same three words, sign in with your
   work.avarok.net account, and type the one-time code the page shows into the terminal.
5. **Redeem the invite** the owner sent you (one long line starting with `swi1.`):

   ```sh
   sw-mcp invite redeem 'swi1.…' --note "hi, it's <your name>"
   ```

   It prints the project, `your_fingerprint` and `owner_fingerprint` (six words each). On a
   call or in chat, read your six words to the owner, and check that the owner's six words
   match `owner_fingerprint`.
6. **Wait for approval.** This returns when the owner has let you in (re-run it if it times out):

   ```sh
   sw-mcp pad wait --after 0 --timeout-s 3600
   ```

   An event with `"kind": "joined"` means you are in. `sw-mcp pad members <project>` now lists
   you both.
7. **Add the MCP server to Claude Code:**

   ```sh
   claude mcp add synchronized-work -- sw-mcp serve --channel
   ```
8. **Start a session** that is woken by notes. With Claude Code channels (research preview):

   ```sh
   MCP_PROTOCOL_NEGOTIATION=legacy claude --dangerously-load-development-channels server:synchronized-work
   ```

   Without channels, start `claude` normally and ask it to keep
   `sw-mcp pad wait --after <cursor> --timeout-s 1500` running under the Monitor tool (or as a
   background command), re-arming it with the cursor of each event.
9. **Post and acknowledge a first note.** Ask Claude: *"post on the synchronized.work
   scratchpad of project `<project>`: hello from <your name>"* (`scratchpad_post`). When the
   owner's note reaches you, Claude shows it to you as data from them, not as an instruction;
   say *"acknowledge it"* (`scratchpad_ack`). From a terminal: `sw-mcp pad inbox` and
   `sw-mcp pad ack <note_id> --reply "thanks"`.

### If you are inviting (the owner)

Do steps 1–4 above yourself, then:

1. **Create the project.** Its scratchpad keeps notes for 7 days.

   ```sh
   sw-mcp project create "Acme refactor"
   ```

   Note the `project` id it prints (`sw-mcp project list` shows it again).
2. **Make an invite for one person** and send it to them privately (it works once, for the
   hours you choose, at most 168):

   ```sh
   sw-mcp invite create <project> --valid-hours 48
   ```

   Send your friend the `invite` line and this section's link.
3. **Approve them.** When they redeem it, your daemon logs it and your Claude Code session is
   woken with a `join_requested` event. List the requests:

   ```sh
   sw-mcp join list
   ```

   Check the request's six `fingerprint` words against the ones your friend reads to you, and
   read them yours (`your_fingerprint`, printed when you made the invite), then:

   ```sh
   sw-mcp join accept <request_id>     # or: sw-mcp join refuse <request_id>
   ```

   In Claude Code the same is `project_join_requests` and `project_join_answer`; Claude
   answers only when you say so. A refused person, or anyone with a spent or expired invite,
   gets nothing.
4. **Say hello:** `scratchpad_post` to the project (or wait for their note and
   `scratchpad_ack` it).

Every write is signed by your Device's ML-DSA-65 key; notes are sealed to the recipients'
ML-KEM-1024 keys, so the node relays them unread. A note from another person reaches your
agent as data from a named, verified person, never as your instruction.

## What is in this repository

| Crate | Kind | Role |
|---|---|---|
| `crates/sw-model` | pure | The tree, the tier state machine, readiness and admission order, visibility, the hashed task embedding. |
| `crates/sw-protocol` | pure | Canonical CBOR `SwMessage`s, the `Signed` (ML-DSA-65) and `Sealed` (ML-KEM-1024 + HKDF-SHA3-256 + AES-256-GCM) envelopes, nonce / `exp` / `seq` rules, the one verification gate, the P2P envelope, the Person record and the `IdentityPort`. Conforms to the frozen batch-1116 `sw-wire` contract. |
| `crates/sw-crypto` | pure | ML-DSA-65, ML-KEM-1024, HKDF-SHA3-256, AES-256-GCM and SHA3-256 behind an injected `Entropy` port. |
| `crates/sw-client` | pure | The ports clients are written against (`SessionPort`, `P2pPort`), the viewer's fold of a Project subscription, and the Sign in with Citadel link. |
| `crates/sw-graph` | pure | The framework-free layout core: tidy-tree positions computed once, spring interpolation, a draw-list IR and hit-testing, shared by the web view and the Citadel App canvas. |
| `crates/sw-testkit` | dev only | Deterministic entropy, the stand-in identity, the fixture cast and the one conformance check. Never a normal dependency (the sbio gate refuses it). |
| `crates/sw-node` | node | The synchronized.work Citadel server (design §3): a `NetKernel` whose pure core verifies every message through sw-protocol's gate, enrols Endpoints by work.avarok.net's assertion, runs the task tree and the sw-model tier transitions, relays prompts through a bounded inbox and pushes `NOTIFY` for significant events. Refuses to start without full provisioning. |
| `crates/sw-store-do` | node | lattice-db's `LatticeStorage` over Durable Object storage: one `put` per key, lossless page ids, `sync` on the object's own, checkpoints from the alarm. Passes lattice-db's conformance suite. |
| `crates/sw-worker` | Worker | The Cloudflare Worker (workers-rs, wasm32-unknown-unknown): routes, `/.well-known/citadel` signed with the node's key, and the `ServerDo` Durable Object that runs the SDK with its accounts in the object's SQL. |
| `crates/sw-view-wasm` | site core | The site's Rust core (wasm-bindgen): folds a Project with sw-client's viewer logic, lays it out once with sw-graph, builds the demo Project from the contract fixtures, and starts Sign in with Citadel. |
| `site/` | site | The public site at https://synchronized.work: React and React Flow as the view only, over `sw-view-wasm`. Served as the Worker's static assets, under the Worker's CSP. |
| `tools/xtask` | tool | CI gates: `file-length`, `sbio`, `conformance`, `plan` (wrangler.toml, including `[assets]`, and deploy.yml), `size`, `site-size` and `node-budget` (against `tools/xtask/budget.toml`). |
| `tools/sw-release-sign` | tool | Signs and verifies the sw-mcp release assets with ML-DSA-65 (the release workflow's signer). |
| `release/` | data | What every sw-mcp release ships beside the binary (`config.toml`, `README.txt`), and the release public key (`sw-mcp-release.pub`). |
| `tools/sw-bench` | tool | A native Citadel client of the node; `tools/measure.sh` drives it against `wrangler dev` for the node budget. |
| `fixtures/` | data | The contract's CBOR fixtures, the sw-wire and sign-in known answers, mirrored byte for byte; every party must decode them identically. `contract.pin` records the parent revision and each file's SHA3-256. |

Pure crates do no I/O and draw no randomness of their own; everything builds for
`wasm32-unknown-unknown` and natively. Cryptography is post-quantum only.

```sh
cargo test --workspace
cargo run -p xtask -- file-length
cargo run -p xtask -- sbio
cargo run -p xtask -- conformance                       # also checks contract.pin
cargo run -p xtask -- conformance --mirror <contract dir>  # against the parent checkout
cargo run -p xtask -- conformance --repin <contract dir> <parent rev>
tools/onboard-e2e.sh   # owner, friend and stranger daemons against `wrangler dev`
```

The site (Node 22; `wasm-bindgen` at the version Cargo.lock pins):

```sh
npm ci --prefix site
SITE_BUILD_SHA=$(git rev-parse HEAD) npm run build --prefix site  # wasm core, typecheck, vite -> site/dist
cargo run -p xtask -- site-size site/dist
npm run test:e2e --prefix site                         # Playwright against `wrangler dev`
npm run brand --prefix site                            # regenerate the icons, og-image, manifest
npm run brand:check --prefix site                      # CI: the derived files match their sources
```

The site's identity has two sources: `site/public/logo.svg` (the mark) and
`site/brand/brand.json` (name, tagline, description, colours). `site/scripts/brand.mjs` derives
every icon, `favicon.ico`, `og-image.png` and `site.webmanifest` from them, and
`site/vite/head.mjs` derives the `<head>` tags; never edit a derived file by hand. The build
needs `SITE_BUILD_SHA`, which versions the service worker's cache.

## Developer scratchpad and async wake-ups

Members of a project leave each other signed notes ("free to merge my PR when you're ready")
through `sw-mcp`'s `scratchpad_post`. Each note is signed by the sender's Device key and sealed
to the recipients' Devices; the node stores and relays it but cannot read or forge it. The
recipient's daemon verifies it against the sender's own enrolment proof and wakes its idle
agent, whose `scratchpad_ack` travels back the same way and wakes the sender.

A note reaches an agent as data from a named, verified person, never as its user's
instruction: from an untrusted sender it is a suggestion to show the user. `sw-mcp pad trust`
marks a colleague's notes as hand-offs, still under the receiving user's own permissions.

A project is created with `sw-mcp project create` (or the `project_create` tool), which
writes the project and its 7-day scratchpad policy, signed by its owner. Others join it only
with an invite: `sw-mcp invite create` gives a one-time `swi1.` code whose token the node
keeps only as a hash; `sw-mcp invite redeem` spends it as a join request; the owner's
`sw-mcp join accept` admits them. Only the owner and admitted members may add Tasks under a
project. See [Getting started](#getting-started).

A session that idles until work arrives:

```sh
sw-mcp daemon &
claude mcp add synchronized-work -- sw-mcp serve --channel
# Push (Claude Code channels, research preview):
MCP_PROTOCOL_NEGOTIATION=legacy claude --dangerously-load-development-channels server:synchronized-work
# Anywhere else, the agent runs this under Monitor or as a background command and re-arms
# it with the cursor each event carries:
sw-mcp pad wait --after 0 --timeout-s 1500
```

## Releases

`sw-mcp` is released by `.github/workflows/release.yml`, from `main` only: when a push to
`main` carries a workspace version with no `sw-mcp-v<version>` tag yet, the workflow builds
macOS arm64 and x64 and Linux x64 (OpenSSL vendored), checks each binary's `--version`,
signs every archive with ML-DSA-65, attests it, verifies all of that, and publishes the
release, tagging the commit it built. Nobody tags by hand; bump the version in a PR to release.
Pull requests that touch the release run it as a dry run with a throwaway key.

Until `release/sw-mcp-release.pub` holds a real key, nothing is published. To enable it:

```sh
cargo run -p sw-release-sign -- keygen --private-key /secure/offline/sw-mcp-release.key \
  --public-key release/sw-mcp-release.pub
# store the private key file's contents as the repository secret SW_MCP_RELEASE_MLDSA_KEY,
# then merge the public key: that merge publishes sw-mcp-v<workspace version>.
```

To check a download yourself:

```sh
sha256sum -c sw-mcp-linux-x64.tar.gz.sha256
gh attestation verify sw-mcp-linux-x64.tar.gz --repo Avarok-Cybersecurity/synchronized.work
cargo run -p sw-release-sign -- verify --public-key release/sw-mcp-release.pub \
  --tag sw-mcp-v0.1.0 sw-mcp-linux-x64.tar.gz
```

## Links

- Avarok Cybersecurity: https://avarok.net
- Author: https://github.com/tbraun96/tbraun96

## Licence

Dual-licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
