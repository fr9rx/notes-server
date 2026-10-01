<#
.SYNOPSIS
  Runs the Cloudflare Tunnel for notes-server as a Windows service on this PC.

.DESCRIPTION
  Run from an elevated PowerShell:

    .\deploy\tunnel-windows.ps1 -TunnelId <id> -CredentialsFile <id>.json

  The tunnel and its DNS record must already exist. Create them with
  `cloudflared tunnel create notes` and
  `cloudflared tunnel route dns notes notes.fr9rx.org`.
  The credentials file is the <id>.json that `tunnel create` wrote.

  Writes C:\ProgramData\cloudflared\notes.yml and installs the `Cloudflared`
  service. The service runs as LocalSystem and starts at boot. Only SYSTEM and
  Administrators can read the folder, because it holds the tunnel secret.
  Requests for -Hostname go to the local notes-server (-Origin). That hop stays
  on this machine, so its self-signed certificate isn't checked.

  Only one machine should run this tunnel at a time. Stop the old host's
  cloudflared first, or Cloudflare splits visitors between the two.
#>
param(
    [Parameter(Mandatory)] [string]$TunnelId,
    [Parameter(Mandatory)] [string]$CredentialsFile,
    [string]$Hostname = 'notes.fr9rx.org',
    [string]$Origin = 'https://127.0.0.1:8443'
)

$ErrorActionPreference = 'Continue'
$name = 'Cloudflared'
$dir = Join-Path $env:ProgramData 'cloudflared'
$config = Join-Path $dir 'notes.yml'
$creds = Join-Path $dir "$TunnelId.json"

function Fail($message) { Write-Host $message -ForegroundColor Red; exit 1 }

$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).
    IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $admin) { Fail 'Run this from an elevated PowerShell (Run as administrator).' }

$cloudflared = (Get-Command cloudflared -ErrorAction SilentlyContinue).Source
if (-not $cloudflared) { Fail 'cloudflared not found. Install it with: winget install Cloudflare.cloudflared' }
if (-not (Test-Path $CredentialsFile)) { Fail "credentials file $CredentialsFile not found" }

New-Item -ItemType Directory -Force $dir | Out-Null
icacls $dir /inheritance:r /grant:r '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' | Out-Null
Copy-Item $CredentialsFile $creds -Force
$yaml = @(
    '# Cloudflare Tunnel for notes-server (deploy/tunnel-windows.ps1).'
    "tunnel: $TunnelId"
    "credentials-file: $creds"
    "logfile: $(Join-Path $dir 'cloudflared.log')"
    'ingress:'
    "  - hostname: $Hostname"
    "    service: $Origin"
    '    originRequest:'
    '      noTLSVerify: true'
    '  - service: http_status:404'
)
[IO.File]::WriteAllText($config, ($yaml -join "`r`n") + "`r`n")
& $cloudflared --config $config tunnel ingress validate
if ($LASTEXITCODE) { Fail "invalid tunnel config $config" }

$imagePath = "`"$cloudflared`" --config `"$config`" tunnel run"
$service = Get-Service $name -ErrorAction SilentlyContinue
if ($service -and $service.Status -ne 'Stopped') {
    Stop-Service $name -Force
    (Get-Service $name).WaitForStatus('Stopped', [TimeSpan]::FromSeconds(30))
}
if (-not $service) {
    New-Service -Name $name -BinaryPathName $imagePath -DisplayName 'Cloudflare Tunnel (notes)' `
        -Description "Cloudflare Tunnel: $Hostname -> $Origin" -StartupType Automatic | Out-Null
}
Set-ItemProperty "HKLM:\SYSTEM\CurrentControlSet\Services\$name" -Name ImagePath -Value $imagePath
sc.exe config $name start= delayed-auto | Out-Null
sc.exe failure $name reset= 86400 actions= restart/5000/restart/10000/restart/60000 | Out-Null
sc.exe failureflag $name 1 | Out-Null

Start-Service $name
Start-Sleep -Seconds 8
$code = curl.exe -s -o NUL -w '%{http_code}' --max-time 10 "https://$Hostname/health"
if ($code -ne '200') {
    Write-Host "https://$Hostname/health answered $code. Last log lines:" -ForegroundColor Red
    Get-Content (Join-Path $dir 'cloudflared.log') -Tail 15 -ErrorAction SilentlyContinue
    exit 1
}
Write-Host "Tunnel up: https://$Hostname -> $Origin" -ForegroundColor Green
