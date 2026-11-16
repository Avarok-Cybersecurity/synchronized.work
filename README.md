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
| `crates/sw-protocol` | pure | Canonical CBOR `SwMessage`s, the `Signed` (ML-DSA-65) and `Sealed` (ML-KEM-768 + HKDF-SHA3-256 + AES-256-GCM) envelopes, nonce / `exp` / `seq` rules, the P2P envelope, the `IdentityPort`. |
| `crates/sw-crypto` | pure | ML-DSA-65, ML-KEM-768, HKDF-SHA3-256, AES-256-GCM behind an injected `Entropy` port. |
| `crates/sw-client` | pure | The ports clients are written against: `SessionPort` and `P2pPort`. |
| `crates/sw-graph` | pure | The framework-free layout core: tidy-tree positions computed once, spring interpolation, a draw-list IR and hit-testing, shared by the web view and the Citadel App canvas. |
| `crates/sw-testkit` | dev only | Deterministic entropy and clock, and the stand-in identity used to build fixtures. Never a normal dependency. |
| `tools/xtask` | tool | CI gates: `file-length`, `sbio`, `conformance`; `gen-fixtures`. |
| `fixtures/` | data | Canonical CBOR vectors every party must decode identically. |

Pure crates do no I/O and draw no randomness of their own; everything builds for
`wasm32-unknown-unknown` and natively. Cryptography is post-quantum only.

```sh
cargo test --workspace
cargo run -p xtask -- file-length
cargo run -p xtask -- sbio
cargo run -p xtask -- conformance
```

## Links

- Avarok Cybersecurity: https://avarok.net
- Author: https://github.com/tbraun96/tbraun96

## Licence

Dual-licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
