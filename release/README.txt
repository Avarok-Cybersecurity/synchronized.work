sw-mcp: the synchronized.work MCP
=================================

sw-mcp      the program (put it on your PATH, e.g. ~/.local/bin)
config.toml the settings for synchronized.work; copy it to ~/.synchronized/config.toml and
            replace DEVICE_SLUG with a name for this machine (lowercase letters, digits, '-').
            The installer (curl -fsSL https://synchronized.work/install.sh | sh) does both.

Then:
  sw-mcp daemon &              keep this running (it is your Device's connection)
  sw-mcp sign-in               sign in with your work.avarok.net account
  sw-mcp invite redeem <code>  join a project you were invited to
  claude mcp add synchronized-work -- sw-mcp serve --channel

Getting started, step by step: https://github.com/Avarok-Cybersecurity/synchronized.work#getting-started
