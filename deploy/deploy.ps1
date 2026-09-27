<#
.SYNOPSIS
  Builds notes-server for the Pi, copies it over SSH, and restarts the service.

.EXAMPLE
  .\deploy\deploy.ps1 -PiHost pi@notes-pi.local -Setup      # first time: installs user, dirs, systemd unit
  .\deploy\deploy.ps1 -PiHost pi@notes-pi.local             # every update after that
  .\deploy\deploy.ps1 -PiHost pi@notes-pi.local -WithBench  # also copy bench_resize to ~/ on the Pi

.NOTES
  Uses the OpenSSH client built into Windows (ssh/scp). Set up key-based login
  once to avoid password prompts: ssh-keygen; then append your .pub key to
  ~/.ssh/authorized_keys on the Pi. sudo on the Pi may still ask for a password.
#>
param(
    [Parameter(Mandatory)][string]$PiHost,
    [switch]$Setup,
    [switch]$NoBuild,
    [switch]$WithBench
)

# Native tools (cargo, ssh) write progress to stderr; PowerShell 5.1 turns that into
# errors under 'Stop' when output is redirected. Failures are detected via $LASTEXITCODE.
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
$release = Join-Path $root 'target\aarch64-unknown-linux-gnu\release'

function Invoke-Checked([string]$what, [scriptblock]$cmd) {
    & $cmd
    if ($LASTEXITCODE) { throw "$what failed (exit $LASTEXITCODE)" }
}

if (-not $NoBuild) {
    & (Join-Path $PSScriptRoot 'build-pi.ps1') -SkipBench:(-not $WithBench)
}
$bin = Join-Path $release 'notes-server'
if (-not (Test-Path $bin)) { throw "$bin not found; run deploy\build-pi.ps1 first" }

if ($Setup) {
    Write-Host '==> One-time Pi setup' -ForegroundColor Cyan
    $files = 'setup-pi.sh', 'notes-server.service', 'notes-server.env.example', 'certbot-deploy-hook.sh' |
        ForEach-Object { Join-Path $PSScriptRoot $_ }
    Invoke-Checked 'ssh mkdir' { ssh $PiHost 'mkdir -p /tmp/notes-setup' }
    Invoke-Checked 'scp setup files' { scp @files "${PiHost}:/tmp/notes-setup/" }
    Invoke-Checked 'setup-pi.sh' { ssh -t $PiHost 'sudo bash /tmp/notes-setup/setup-pi.sh && rm -rf /tmp/notes-setup' }
}

Write-Host '==> Uploading binary' -ForegroundColor Cyan
Invoke-Checked 'scp binary' { scp $bin "${PiHost}:/tmp/notes-server.new" }
if ($WithBench) {
    Invoke-Checked 'scp bench' { scp (Join-Path $release 'examples\bench_resize') "${PiHost}:bench_resize" }
}

Write-Host '==> Installing and restarting' -ForegroundColor Cyan
# `install` replaces the file (new inode), so it works while the old binary is running.
$remote = 'sudo install -m 755 -o root -g root /tmp/notes-server.new /opt/notes-server/notes-server' +
    ' && rm -f /tmp/notes-server.new' +
    ' && sudo systemctl restart notes-server' +
    ' && sleep 2 && systemctl is-active notes-server' +
    ' || { sudo journalctl -u notes-server -n 30 --no-pager; exit 1; }'
Invoke-Checked 'remote install/restart' { ssh -t $PiHost $remote }

Write-Host 'Deployed. Logs: ssh' $PiHost "'journalctl -u notes-server -f'" -ForegroundColor Green
