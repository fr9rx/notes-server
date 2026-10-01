<#
.SYNOPSIS
  Installs or updates notes-server as a Windows service on this PC.

.DESCRIPTION
  Run from an elevated PowerShell in the repository:

    .\deploy\install-windows.ps1 -PublicUrl https://notes.fr9rx.org

  1. Builds the website and the release binary (skip with -NoBuild).
  2. Installs notes-server.exe into C:\Program Files\notes-server.
  3. First time only, under C:\ProgramData\notes-server:
     - data\, uploads\ and logs\;
     - notes-server.env, with a new random ADMIN_TOKEN;
     - certs\, a self-signed certificate for the local HTTPS listener.
     The certificate is fine behind a Cloudflare Tunnel, which serves the
     public certificate itself.
  4. Registers the `notes-server` service:
     - starts automatically at boot, before anyone signs in;
     - restarts after a crash;
     - runs as its own virtual account, NT SERVICE\notes-server. It can read
       its settings and write only data\, uploads\ and logs\.
  5. Starts the service and checks /health.

  Running it again updates the binary and keeps the data, token and certificate.
  Pass -PublicUrl again to change PUBLIC_BASE_URL.
#>
param(
    [string]$PublicUrl = 'https://localhost:8443',
    [string]$BindAddr = '127.0.0.1:8443',
    [switch]$NoBuild
)

# Native tools write progress to stderr; PowerShell 5.1 turns that into errors
# under 'Stop' when output is redirected. Failures are detected via $LASTEXITCODE.
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
$name = 'notes-server'
$account = "NT SERVICE\$name"
$installDir = Join-Path $env:ProgramFiles $name
$exe = Join-Path $installDir 'notes-server.exe'
$home_ = Join-Path $env:ProgramData $name
$envFile = Join-Path $home_ 'notes-server.env'
$certDir = Join-Path $home_ 'certs'

function Fail($message) { Write-Host $message -ForegroundColor Red; exit 1 }

$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).
    IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) { Fail 'Run this from an elevated PowerShell (Run as administrator).' }

# ---- Build -------------------------------------------------------------------
if (-not $NoBuild) {
    if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
        Fail 'npm not found on PATH. Install Node.js LTS (see README "Laptop setup").'
    }
    Push-Location (Join-Path $root 'frontend')
    try {
        if (-not (Test-Path 'node_modules')) {
            npm ci --no-audit --no-fund
            if ($LASTEXITCODE) { Fail 'npm ci failed' }
        }
        npm run build
        if ($LASTEXITCODE) { Fail 'building the frontend failed' }
    }
    finally { Pop-Location }

    Push-Location $root
    try {
        cargo build --release --bin notes-server
        if ($LASTEXITCODE) { Fail 'building notes-server failed' }
    }
    finally { Pop-Location }
}
$built = Join-Path $root 'target\release\notes-server.exe'
if (-not (Test-Path $built)) { Fail "$built not found; run without -NoBuild." }

# ---- Install the binary --------------------------------------------------------
$service = Get-Service $name -ErrorAction SilentlyContinue
if ($service -and $service.Status -ne 'Stopped') {
    Write-Host "==> Stopping $name"
    Stop-Service $name -Force
    (Get-Service $name).WaitForStatus('Stopped', [TimeSpan]::FromSeconds(45))
}
New-Item -ItemType Directory -Force $installDir | Out-Null
Copy-Item $built $exe -Force
Write-Host "==> Installed $exe"

# ---- Data folders, settings, certificate (first time) --------------------------
foreach ($dir in 'data', 'uploads', 'logs', 'certs') {
    New-Item -ItemType Directory -Force (Join-Path $home_ $dir) | Out-Null
}
$fwd = { param($p) $p -replace '\\', '/' }
if (-not (Test-Path $envFile)) {
    $bytes = New-Object byte[] 36
    [Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($bytes)
    $token = [Convert]::ToBase64String($bytes).Replace('+', '-').Replace('/', '_')
    $settings = @(
        '# notes-server settings, read by the Windows service (--env-file).'
        '# Restart the service after editing: Restart-Service notes-server'
        "ADMIN_TOKEN=$token"
        "BIND_ADDR=$BindAddr"
        "PUBLIC_BASE_URL=$PublicUrl"
        "TLS_CERT_PATH=$certDir\cert.pem"
        "TLS_KEY_PATH=$certDir\key.pem"
        "DATABASE_URL=sqlite://$(& $fwd (Join-Path $home_ 'data\notes.db'))?mode=rwc"
        "UPLOAD_DIR=$(Join-Path $home_ 'uploads')"
        "LOG_DIR=$(Join-Path $home_ 'logs')"
    )
    # UTF-8 without a BOM.
    [IO.File]::WriteAllText($envFile, ($settings -join "`r`n") + "`r`n")
    Write-Host "==> Wrote $envFile with a new ADMIN_TOKEN"
}
elseif ($PSBoundParameters.ContainsKey('PublicUrl')) {
    $lines = [IO.File]::ReadAllLines($envFile) | ForEach-Object {
        if ($_ -match '^PUBLIC_BASE_URL=') { "PUBLIC_BASE_URL=$PublicUrl" } else { $_ }
    }
    [IO.File]::WriteAllText($envFile, ($lines -join "`r`n") + "`r`n")
    Write-Host "==> PUBLIC_BASE_URL=$PublicUrl"
}
if (-not (Test-Path (Join-Path $certDir 'cert.pem'))) {
    & $exe self-signed-cert --cert (Join-Path $certDir 'cert.pem') --key (Join-Path $certDir 'key.pem') `
        localhost 127.0.0.1 $env:COMPUTERNAME
    if ($LASTEXITCODE) { Fail 'creating the self-signed certificate failed' }
}

# ---- Service ---------------------------------------------------------------------
$imagePath = "`"$exe`" --service --env-file `"$envFile`""
if (-not $service) {
    New-Service -Name $name -BinaryPathName $imagePath -DisplayName 'notes-server' `
        -Description 'Notes website and API (HTTPS). Settings: C:\ProgramData\notes-server\notes-server.env' `
        -StartupType Automatic | Out-Null
    Write-Host "==> Registered the $name service"
}
# Set every time, so updates also fix older registrations. (sc.exe can't take
# the quoted binPath reliably from PowerShell 5.1; the registry can.)
Set-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Services\$name" -Name ImagePath -Value $imagePath
sc.exe config $name obj= $account start= delayed-auto | Out-Null
if ($LASTEXITCODE) { Fail "configuring the $name service account failed" }
# Restart after a crash or an error exit: 5 s, 10 s, then every minute; reset after a day.
sc.exe failure $name reset= 86400 actions= restart/5000/restart/10000/restart/60000 | Out-Null
sc.exe failureflag $name 1 | Out-Null

# ---- Permissions -------------------------------------------------------------------
# Only SYSTEM and Administrators get full control. The service account can read
# its settings and certificate, and write only data, uploads and logs.
icacls $home_ /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' "${account}:(OI)(CI)RX" | Out-Null
if ($LASTEXITCODE) { Fail "setting permissions on $home_ failed" }
foreach ($dir in 'data', 'uploads', 'logs') {
    icacls (Join-Path $home_ $dir) /grant:r "${account}:(OI)(CI)M" | Out-Null
}

# ---- Start and check ----------------------------------------------------------------
Write-Host "==> Starting $name"
Start-Service $name
$port = ($BindAddr -split ':')[-1]
$ok = $false
foreach ($i in 1..20) {
    Start-Sleep -Milliseconds 500
    $health = curl.exe -sk --max-time 3 "https://127.0.0.1:$port/health" 2>$null
    if ($health -eq 'ok') { $ok = $true; break }
}
if (-not $ok) {
    Write-Host "The service did not answer on https://127.0.0.1:$port/health. Latest log lines:" -ForegroundColor Red
    Get-ChildItem (Join-Path $home_ 'logs') -Filter 'notes-server*.log' -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime | Select-Object -Last 1 | ForEach-Object { Get-Content $_.FullName -Tail 20 }
    exit 1
}
$public = (Select-String '^PUBLIC_BASE_URL=' $envFile).Line -replace '^PUBLIC_BASE_URL=', ''
Write-Host "Running: https://127.0.0.1:$port (public: $public)" -ForegroundColor Green
Write-Host "Admin token: Select-String ADMIN_TOKEN '$envFile'   Logs: $home_\logs"
