#!/usr/bin/env bash
# A native test node and a link code, as another repository's CI runs them (no Docker, no
# wrangler, nothing reached beyond loopback): build sw-devnode and sw-mcp, start the node, write
# a signed-in home for a test Person, run the real daemon, create a project and mint a code.
#   tools/devnode-e2e.sh [port]
# Prints the node URL and the link-code JSON. Test provisioning only: published seeds.
set -euo pipefail
cd "$(dirname "$0")/.."
port=${1:-18790}
cargo build --locked -p sw-devnode -p sw-mcp
bin=${CARGO_TARGET_DIR:-target}/debug
work=$(mktemp -d)
pids=()
finish() {
  status=$?
  if [ "$status" -ne 0 ]; then tail -50 "$work"/*.log || true; fi
  for p in "${pids[@]}"; do kill "$p" 2>/dev/null || true; done
  rm -rf "$work"
}
trap finish EXIT
"$bin/sw-devnode" serve "$port" > "$work/node.log" 2>&1 &
pids+=($!)
for _ in $(seq 60); do grep -q '^ready ' "$work/node.log" && break; sleep 1; done
grep '^ready ' "$work/node.log" || { echo "the node never became ready"; exit 1; }
"$bin/sw-devnode" home "$work/owner" "$port" "$(printf 'a1%.0s' $(seq 32))"
"$bin/sw-mcp" --home "$work/owner" daemon > "$work/daemon.log" 2>&1 &
pids+=($!)
created=
for _ in $(seq 60); do
  created=$("$bin/sw-mcp" --home "$work/owner" project create Acme 2>/dev/null) && break
  sleep 2
done
[ -n "$created" ] || { echo "the daemon never reached the node"; exit 1; }
project=$(printf '%s' "$created" | node -p 'JSON.parse(require("fs").readFileSync(0)).project')
"$bin/sw-mcp" --home "$work/owner" link-code --project "$project"
