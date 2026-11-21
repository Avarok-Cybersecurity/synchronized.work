#!/usr/bin/env bash
# The onboarding path end to end against the real Worker: start `wrangler dev` on a fresh local
# state, provisioned with the C0 KAT sign-in issuer the test homes pin, then run sw-mcp's
# `onboard_wrangler` test (production daemons for an owner, a friend and a stranger) against it.
#   tools/onboard-e2e.sh
# Test provisioning only: every seed below is a published test value, never a production key.
set -euo pipefail
cd "$(dirname "$0")/.."
port=8788
issuer_seed=$(node -p "require('./fixtures/sign-in-kat.json').issuer.key.seed_hex")
cargo build --locked -p sw-bench
bench=${CARGO_TARGET_DIR:-target}/debug/sw-bench
cargo test --locked -p sw-mcp --test onboard_wrangler --no-run
state=$(mktemp -d)
vars=$(mktemp)
log=$(mktemp)
dev=
finish() {
  status=$?
  if [ "$status" -ne 0 ]; then echo "--- wrangler dev log (last 200 lines)"; tail -200 "$log"; fi
  [ -z "$dev" ] || { kill "$dev" 2>/dev/null || true; wait "$dev" 2>/dev/null || true; }
  rm -rf "$state" "$vars" "$log"
}
trap finish EXIT
{
  echo "SW_SERVER_KEY_SEED=$(printf '11%.0s' $(seq 32))"
  echo "SW_OPRF_SEED=$(printf '22%.0s' $(seq 32))"
  echo "SW_SERVER_KEY_ISSUED_AT=1794700000"
  echo "SW_SIGN_IN_ISSUER_PK=$("$bench" issuer-pk "$issuer_seed")"
  echo "SW_BUILD_SHA=onboard-e2e"
  echo "LOG_LEVEL=info"
} > "$vars"
# The Worker's wasm build alone takes getrandom's browser backend (see ci.yml's worker job); the
# native builds above and the test below run with the caller's RUSTFLAGS.
RUSTFLAGS="${RUSTFLAGS:-} --cfg getrandom_backend=\"wasm_js\"" \
  npx wrangler dev --port "$port" --ip 127.0.0.1 --local --persist-to "$state" --env-file "$vars" > "$log" 2>&1 &
dev=$!
for _ in $(seq 900); do
  grep -q "Ready on" "$log" && break
  kill -0 "$dev" 2>/dev/null || { cat "$log"; exit 1; }
  sleep 1
done
grep -q "Ready on" "$log" || { cat "$log"; echo "wrangler dev never became ready"; exit 1; }
health="$(curl -fsS "http://127.0.0.1:$port/healthz" || true)"
case "$health" in
  *'"node":"ready"'*'"sha":"onboard-e2e"'*) ;;
  *) echo "/healthz answered ${health:-nothing}: not this provisioned build"; exit 1 ;;
esac
SW_E2E_WRANGLER_PORT="$port" cargo test --locked -p sw-mcp --test onboard_wrangler -- --ignored --nocapture
