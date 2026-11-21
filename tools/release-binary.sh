#!/usr/bin/env bash
# The release workflow's per-platform steps on one built sw-mcp (.github/workflows/release.yml):
#   tools/release-binary.sh rustflags <target>           RUSTFLAGS=… mapping the checkout and
#                                                        cargo's home out of every Rust path
#   tools/release-binary.sh check   <target> <version>   it is this version, for this machine
#                                                        type, linking no host OpenSSL, and it
#                                                        names no build-machine home directory
#   tools/release-binary.sh package <target> <asset>     <asset> (.tar.gz or .zip) + .sha256
# Runs under bash on every runner (Git Bash on Windows).
set -euo pipefail
cd "$(dirname "$0")/.."
cmd=$1 target=$2
exe=""; [ "${RUNNER_OS:-}" = Windows ] && exe=.exe
bin="target/$target/release/sw-mcp$exe"

# Prefixes rustc writes into panic locations and `file!()`: this checkout and cargo's home.
rustflags() {
  local root=$PWD cargo_home=${CARGO_HOME:-$HOME/.cargo}
  if [ "${RUNNER_OS:-}" = Windows ]; then root="$(cygpath -w "$root")"; cargo_home="$(cygpath -w "$cargo_home")"; fi
  case "$root$cargo_home" in *" "*) echo "::error::a path with a space cannot be remapped" >&2; exit 1 ;; esac
  echo "RUSTFLAGS=--remap-path-prefix=$root=/sw --remap-path-prefix=$cargo_home=/cargo"
}

# Prints a tool's whole report, then fails if it matches. The report is captured first: piping
# it through `tee /dev/stderr | grep -q` hid every match (Git Bash has no /dev/stderr, and under
# pipefail a failing tee, or grep -q closing the pipe early, sent the match to `|| true`).
refuse_match() {
  local report=$1 pattern=$2 why=$3
  printf '%s\n' "$report"
  if grep -qiE "$pattern" <<<"$report"; then echo "::error::$why"; exit 1; fi
}

check() {
  local version=$1
  [ "$("$bin" --version | tr -d '\r')" = "sw-mcp $version" ] || { "$bin" --version; echo "::error::not sw-mcp $version"; exit 1; }
  case "${RUNNER_OS:-}" in
    macOS)
      refuse_match "$(otool -L "$bin")" 'openssl|libssl|libcrypto|/opt/homebrew|/usr/local' "links a host library"
      [ "$(lipo -archs "$bin")" = "$([ "$target" = x86_64-apple-darwin ] && echo x86_64 || echo arm64)" ] || { echo "::error::wrong architecture"; exit 1; } ;;
    Windows)
      local vswhere="/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe" dumpbin
      dumpbin="$("$vswhere" -latest -products '*' -find 'VC\Tools\MSVC\**\bin\Hostx64\x64\dumpbin.exe' | head -1 | tr -d '\r')"
      [ -n "$dumpbin" ] || { echo "::error::dumpbin.exe not found"; exit 1; }
      refuse_match "$("$dumpbin" -nologo -dependents "$bin")" 'libssl|libcrypto|openssl' "links an OpenSSL DLL"
      grep -q 'machine (x64)' <<<"$("$dumpbin" -nologo -headers "$bin")" || { echo "::error::not an x64 binary"; exit 1; } ;;
    *)
      refuse_match "$(ldd "$bin")" 'libssl|libcrypto' "links the host OpenSSL" ;;
  esac
  # Vendored OpenSSL compiles its own build directory in (ENGINESDIR, MODULESDIR); no rustc flag
  # reaches C, so that one directory, under target/, is allowed. Nothing else is.
  cargo run --locked -q -p xtask -- home-paths \
    --allow-under "$PWD/target/$target/release/build/openssl-sys-" "$bin"
}

package() {
  local asset=$1 staging
  staging="$(mktemp -d)"
  cp "$bin" release/config.toml release/README.txt LICENSE-MIT LICENSE-APACHE "$staging/"
  case "$asset" in
    *.zip) (cd "$staging" && 7z a -tzip -bd "$OLDPWD/$asset" . >/dev/null) ;;
    *.tar.gz) tar -czf "$asset" -C "$staging" . ;;
    *) echo "::error::unknown archive type: $asset"; exit 1 ;;
  esac
  if command -v sha256sum >/dev/null; then sha256sum "$asset"; else shasum -a 256 "$asset"; fi > "$asset.sha256"
}

case "$cmd" in
  rustflags) rustflags ;;
  check) check "$3" ;;
  package) package "$3" ;;
  *) echo "usage: $0 rustflags <target> | check <target> <version> | package <target> <asset>" >&2; exit 2 ;;
esac
