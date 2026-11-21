# Installs sw-mcp, the synchronized.work MCP, on Windows from its latest GitHub release. In
# PowerShell (no administrator needed):
#   irm https://synchronized.work/install.ps1 | iex
# It downloads the Windows archive, checks its SHA-256 and (when the GitHub CLI is installed)
# its build attestation, puts sw-mcp.exe in %LOCALAPPDATA%\Programs\sw-mcp (or
# $env:SW_MCP_BIN_DIR) and on your user PATH, and writes
# %LOCALAPPDATA%\synchronized\config.toml if there is none yet. The counterpart of install.sh;
# the asset name is the one .github/workflows/release.yml publishes.

& {
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

    $repo = 'Avarok-Cybersecurity/synchronized.work'
    $base = "https://github.com/$repo/releases/latest/download"
    $asset = 'sw-mcp-windows-x64.zip'

    # x64; Windows on Arm runs the x64 build under its emulation.
    if ($env:PROCESSOR_ARCHITECTURE -notin @('AMD64', 'ARM64')) {
        throw "sw-mcp has no build for $($env:PROCESSOR_ARCHITECTURE) Windows."
    }
    if (-not $env:LOCALAPPDATA) { throw 'LOCALAPPDATA is not set.' }

    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("sw-mcp-" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        Write-Host "Downloading $asset"
        $zip = Join-Path $tmp $asset
        Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset" -OutFile $zip
        Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset.sha256" -OutFile "$zip.sha256"

        # The .sha256 is `<hex>  <name>` (or `<hex> *<name>`); compare the hex alone.
        $expected = ((Get-Content -Raw "$zip.sha256").Trim() -split '\s+')[0].ToLowerInvariant()
        $actual = (Get-FileHash -Algorithm SHA256 -Path $zip).Hash.ToLowerInvariant()
        if ($expected -notmatch '^[0-9a-f]{64}$' -or $actual -ne $expected) {
            throw "Checksum mismatch for ${asset}: expected $expected, got $actual. Nothing was installed."
        }
        Write-Host "${asset}: checksum OK"

        if (Get-Command gh -ErrorAction SilentlyContinue) {
            gh attestation verify $zip --repo $repo
            if ($LASTEXITCODE -ne 0) { throw 'The build attestation did not verify. Nothing was installed.' }
        } else {
            Write-Host 'note: the checksum matches; install the GitHub CLI (gh) to also verify where it was built.'
        }

        $x = Join-Path $tmp 'x'
        Expand-Archive -Path $zip -DestinationPath $x
        $dest = if ($env:SW_MCP_BIN_DIR) { $env:SW_MCP_BIN_DIR } else { Join-Path $env:LOCALAPPDATA 'Programs\sw-mcp' }
        New-Item -ItemType Directory -Force -Path $dest | Out-Null
        Copy-Item (Join-Path $x 'sw-mcp.exe') (Join-Path $dest 'sw-mcp.exe.new') -Force
        Move-Item (Join-Path $dest 'sw-mcp.exe.new') (Join-Path $dest 'sw-mcp.exe') -Force
        $version = & (Join-Path $dest 'sw-mcp.exe') --version
        Write-Host "Installed $dest\sw-mcp.exe ($version)"

        $stateDir = Join-Path $env:LOCALAPPDATA 'synchronized'
        $config = Join-Path $stateDir 'config.toml'
        if (Test-Path $config) {
            Write-Host "Kept your existing $config"
        } else {
            $slug = ($env:COMPUTERNAME.ToLowerInvariant() -replace '[^a-z0-9-]', '-').TrimStart('-')
            if ($slug.Length -gt 64) { $slug = $slug.Substring(0, 64) }
            if (-not $slug) { $slug = 'device' }
            New-Item -ItemType Directory -Force -Path $stateDir | Out-Null
            $text = (Get-Content -Raw (Join-Path $x 'config.toml')).Replace('DEVICE_SLUG', $slug)
            [IO.File]::WriteAllText($config, $text, (New-Object Text.UTF8Encoding $false))
            Write-Host "Wrote $config (this machine is `"$slug`")"
        }

        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        $entries = @($userPath -split ';' | Where-Object { $_ })
        if ($entries -notcontains $dest) {
            [Environment]::SetEnvironmentVariable('Path', (($entries + $dest) -join ';'), 'User')
            Write-Host "Added $dest to your user PATH; open a new terminal for it to take effect."
        }
        if (($env:Path -split ';') -notcontains $dest) { $env:Path = "$env:Path;$dest" }
        Write-Host "Next: run 'sw-mcp daemon' in a terminal you keep open, then 'sw-mcp sign-in' in another."
    } finally {
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }
}
