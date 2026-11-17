// Builds the view core (crates/sw-view-wasm) for wasm32-unknown-unknown and generates its
// bindings into src/wasm/pkg (git-ignored). wasm-bindgen must be the version Cargo.lock pins.
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const site = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
const repo = path.dirname(site);
const target = process.env.CARGO_TARGET_DIR ?? path.join(repo, 'target');
const run = (cmd, args) => execFileSync(cmd, args, { cwd: repo, stdio: 'inherit' });

run('cargo', ['build', '--locked', '--release', '--target', 'wasm32-unknown-unknown', '-p', 'sw-view-wasm']);
run('wasm-bindgen', [
  '--target', 'web',
  '--out-dir', path.join(site, 'src', 'wasm', 'pkg'),
  path.join(target, 'wasm32-unknown-unknown', 'release', 'sw_view_wasm.wasm'),
]);
