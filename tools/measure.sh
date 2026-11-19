#!/usr/bin/env bash
# Measure the node under `wrangler dev` (design §11.1 node-budget): build, start the Worker on
# a fresh local state with test provisioning, drive it with sw-bench, write the figures.
#   tools/measure.sh <sessions> <messages> <out.json>
# Test provisioning only: every seed below is a published test value, never a production key.
set -euo pipefail
cd "$(dirname "$0")/.."
sessions="$1"; messages="$2"; out="$3"
issuer_seed=$(printf '51%.0s' $(seq 32))
cargo build --release --locked -p sw-bench
bench=${CARGO_TARGET_DIR:-target}/release/sw-bench
state=$(mktemp -d)
vars=$(mktemp)
trap 'kill "$dev" 2>/dev/null || true; wait "$dev" 2>/dev/null || true; rm -rf "$state" "$vars"' EXIT
{
  echo "SW_SERVER_KEY_SEED=$(printf '11%.0s' $(seq 32))"
  echo "SW_OPRF_SEED=$(printf '22%.0s' $(seq 32))"
  echo "SW_SERVER_KEY_ISSUED_AT=1794700000"
  echo "SW_SIGN_IN_ISSUER_PK=$("$bench" issuer-pk "$issuer_seed")"
  echo "SW_BUILD_SHA=measure"
  echo "LOG_LEVEL=info"
} > "$vars"
log=$(mktemp)
npx wrangler dev --port 8787 --ip 127.0.0.1 --local --persist-to "$state" --env-file "$vars" > "$log" 2>&1 &
dev=$!
for _ in $(seq 600); do
  grep -q "Ready on" "$log" && break
  kill -0 "$dev" 2>/dev/null || { cat "$log"; exit 1; }
  sleep 1
done
grep -q "Ready on" "$log" || { cat "$log"; echo "wrangler dev never became ready"; exit 1; }
[ "$(curl -fsS http://127.0.0.1:8787/healthz)" = measure ]
"$bench" run ws://127.0.0.1:8787/citadel "$issuer_seed" "$sessions" "$messages" "$out"
# The ServerDO logs its wasm memory at every accepted socket; the last line is the figure at
# `sessions` live sessions.
memory=$(grep -o 'wasm_memory_bytes=[0-9]*' "$log" | tail -1 | cut -d= -f2)
[ -n "$memory" ] || { echo "the node logged no wasm_memory_bytes"; exit 1; }
node -e 'const fs=require("fs");const [f,m]=process.argv.slice(1);const r=JSON.parse(fs.readFileSync(f));r.wasm_memory_bytes_at_sessions=Number(m);fs.writeFileSync(f,JSON.stringify(r));console.log(JSON.stringify(r))' "$out" "$memory"
