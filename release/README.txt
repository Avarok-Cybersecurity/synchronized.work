sw-mcp: the synchronized.work MCP
=================================

sw-mcp      the program (sw-mcp.exe on Windows; put it on your PATH, e.g. ~/.local/bin)
config.toml the settings for synchronized.work; copy it to ~/.synchronized/config.toml
            (Windows: %LOCALAPPDATA%\synchronized\config.toml) and replace DEVICE_SLUG with a
            name for this machine (lowercase letters, digits, '-'). The installers do both:
              macOS, Linux:  curl -fsSL https://synchronized.work/install.sh | sh
              Windows:       irm https://synchronized.work/install.ps1 | iex   (PowerShell)

Then:
  sw-mcp daemon &              keep this running (it is your Device's connection; on Windows,
                               run `sw-mcp daemon` in a PowerShell window you leave open)
  sw-mcp sign-in               sign in with your work.avarok.net account
  sw-mcp invite redeem <code>  join a project you were invited to
  claude mcp add synchronized-work -- sw-mcp serve --channel

Getting started, step by step: https://github.com/Avarok-Cybersecurity/synchronized.work#getting-started
