#!/bin/sh
# Installs sw-mcp, the synchronized.work MCP, from its latest GitHub release:
#   curl -fsSL https://synchronized.work/install.sh | sh
# It downloads the archive for this machine, checks its SHA-256 and (when the GitHub CLI is
# installed) its build attestation, puts sw-mcp in ~/.local/bin (or $SW_MCP_BIN_DIR), and
# writes ~/.synchronized/config.toml if there is none yet. Nothing runs as root.
# The asset names are the ones .github/workflows/release.yml publishes.
set -eu

repo="Avarok-Cybersecurity/synchronized.work"
base="https://github.com/$repo/releases/latest/download"

case "$(uname -s)/$(uname -m)" in
  Darwin/arm64) asset=sw-mcp-macos-arm64.tar.gz ;;
  Darwin/x86_64) asset=sw-mcp-macos-x64.tar.gz ;;
  Linux/x86_64) asset=sw-mcp-linux-x64.tar.gz ;;
  MINGW*|MSYS*|CYGWIN*)
    echo "On Windows, install the native build from PowerShell: irm https://synchronized.work/install.ps1 | iex" >&2
    exit 1
    ;;
  *)
    echo "sw-mcp has no build for $(uname -s) $(uname -m) yet." >&2
    exit 1
    ;;
esac

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM
echo "Downloading $asset"
curl -fsSL -o "$tmp/$asset" "$base/$asset"
curl -fsSL -o "$tmp/$asset.sha256" "$base/$asset.sha256"

if command -v shasum >/dev/null 2>&1; then
  (cd "$tmp" && shasum -a 256 -c "$asset.sha256")
else
  (cd "$tmp" && sha256sum -c "$asset.sha256")
fi

if command -v gh >/dev/null 2>&1; then
  gh attestation verify "$tmp/$asset" --repo "$repo"
else
  echo "note: the checksum matches; install the GitHub CLI (gh) to also verify where it was built."
fi

mkdir -p "$tmp/x"
tar -xzf "$tmp/$asset" -C "$tmp/x"
dest="${SW_MCP_BIN_DIR:-$HOME/.local/bin}"
mkdir -p "$dest"
cp "$tmp/x/sw-mcp" "$dest/sw-mcp.new"
chmod 0755 "$dest/sw-mcp.new"
mv "$dest/sw-mcp.new" "$dest/sw-mcp"
echo "Installed $dest/sw-mcp ($("$dest/sw-mcp" --version))"

home="$HOME/.synchronized"
if [ -f "$home/config.toml" ]; then
  echo "Kept your existing $home/config.toml"
else
  slug="$(hostname | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9-]/-/g; s/^-*//' | cut -c1-64)"
  [ -n "$slug" ] || slug=device
  mkdir -p "$home"
  chmod 0700 "$home"
  sed "s/DEVICE_SLUG/$slug/" "$tmp/x/config.toml" > "$home/config.toml"
  chmod 0600 "$home/config.toml"
  echo "Wrote $home/config.toml (this machine is \"$slug\")"
fi

case ":$PATH:" in
  *":$dest:"*) ;;
  *) echo "Add $dest to your PATH, e.g.: echo 'export PATH=\"$dest:\$PATH\"' >> ~/.profile" ;;
esac
echo "Next: run 'sw-mcp daemon' in a terminal you keep open, then 'sw-mcp sign-in'."
