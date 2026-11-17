# synchronized.work

**synchronized.work is an open-source coordination service by [Avarok Cybersecurity](https://avarok.net) that lets many people's coding agents share one live task tree and receive post-quantum signed prompts from anywhere.**

It extends the "lanes" model — one coordinator admitting parallel agent lanes, readiest first,
with declared blockers — from one person's machine to many people. Each Project is a tree
(Project → Task → Person → Agent → Progress); each Task has a cooperation tier (`off`, `aware`,
`synchronized`) that only its owner opens or closes, by signature.

## What is in this repository

| Crate | Kind | Role |
|---|---|---|
| `crates/sw-model` | pure | The tree, the tier state machine, readiness and admission order, visibility, the hashed task embedding. |
| `crates/sw-protocol` | pure | Canonical CBOR `SwMessage`s, the `Signed` (ML-DSA-65) and `Sealed` (ML-KEM-768 + HKDF-SHA3-256 + AES-256-GCM) envelopes, nonce / `exp` / `seq` rules, the one verification gate, the P2P envelope, the Person record and the `IdentityPort`. Conforms to the frozen batch-1116 `sw-wire` contract. |
| `crates/sw-crypto` | pure | ML-DSA-65, ML-KEM-768, HKDF-SHA3-256, AES-256-GCM and SHA3-256 behind an injected `Entropy` port. |
| `crates/sw-client` | pure | The ports clients are written against: `SessionPort` and `P2pPort`. |
| `crates/sw-graph` | pure | The framework-free layout core: tidy-tree positions computed once, spring interpolation, a draw-list IR and hit-testing, shared by the web view and the Citadel App canvas. |
| `crates/sw-testkit` | dev only | Deterministic entropy, the stand-in identity, the fixture cast and the one conformance check. Never a normal dependency (the sbio gate refuses it). |
| `crates/sw-node` | node | The synchronized.work Citadel server (design §3): a `NetKernel` whose pure core verifies every message through sw-protocol's gate, enrols Endpoints by work.avarok.net's assertion, runs the task tree and the sw-model tier transitions, relays prompts through a bounded inbox and pushes `NOTIFY` for significant events. Refuses to start without full provisioning. |
| `crates/sw-store-do` | node | lattice-db's `LatticeStorage` over Durable Object storage: one `put` per key, lossless page ids, `sync` on the object's own, checkpoints from the alarm. Passes lattice-db's conformance suite. |
| `crates/sw-worker` | Worker | The Cloudflare Worker (workers-rs, wasm32-unknown-unknown): routes, `/.well-known/citadel` signed with the node's key, and the `ServerDo` Durable Object that runs the SDK with its accounts in the object's SQL. |
| `tools/xtask` | tool | CI gates: `file-length`, `sbio`, `conformance`, `plan` (wrangler.toml and deploy.yml), `size` and `node-budget` (against `tools/xtask/budget.toml`). |
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
```

## Links

- Avarok Cybersecurity: https://avarok.net
- Author: https://github.com/tbraun96/tbraun96

## Licence

Dual-licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
