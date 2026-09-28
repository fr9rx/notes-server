<#
.SYNOPSIS
  Builds and deploys notes-server + the LED matrix firmware to an Arduino UNO Q over adb (USB).

.EXAMPLE
  .\deploy\deploy-unoq.ps1 -Setup         # first time: installs the service (asks for the board's sudo password once)
  .\deploy\deploy-unoq.ps1                # build + deploy server and firmware
  .\deploy\deploy-unoq.ps1 -NoFirmware    # server only
  .\deploy\deploy-unoq.ps1 -NoBuild       # deploy what was built last time
  .\deploy\deploy-unoq.ps1 -RestoreArduinoLoader   # put Arduino's sketch loader back on the STM32

.NOTES
  adb comes with the Android SDK platform-tools or Arduino App Lab; pass -Adb if it
  isn't on PATH. With several devices attached, pass -Serial (see `adb devices`).
#>
param(
    [switch]$Setup,
    [switch]$NoBuild,
    [switch]$NoFirmware,
    [switch]$RestoreArduinoLoader,
    [string]$Serial,
    [string]$Adb
)

$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
$bin = Join-Path $root 'target\aarch64-unknown-linux-gnu\release\notes-server'
$firmware = Join-Path $root 'mcu\build\zephyr\zephyr.elf'
$flashCfg = Join-Path $root 'mcu\notes-matrix\flash.cfg'
# remoteocd ships with the board's Arduino core; it drives OpenOCD over the board's GPIO SWD lines.
$remoteocd = '~/.arduino15/packages/arduino/tools/remoteocd/*/remoteocd'
$arduinoLoader = '~/.arduino15/packages/arduino/hardware/zephyr/*/firmwares/zephyr-arduino_uno_q_stm32u585xx.elf'

if (-not $Adb) {
    $candidates = @(
        (Get-Command adb -ErrorAction SilentlyContinue).Source,
        "$env:LOCALAPPDATA\Android\Sdk\platform-tools\adb.exe",
        "$env:LOCALAPPDATA\Arduino15\packages\arduino\tools\adb\32.0.0\adb.exe"
    )
    $Adb = $candidates | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
    if (-not $Adb) { throw 'adb not found; install Android platform-tools or pass -Adb <path to adb.exe>' }
}
$adbArgs = @()
if ($Serial) { $adbArgs = @('-s', $Serial) }

function Invoke-Adb {
    param([string]$what)
    & $Adb @adbArgs @args
    if ($LASTEXITCODE) { throw "$what failed (exit $LASTEXITCODE)" }
}

$state = (& $Adb @adbArgs get-state 2>$null)
if ($state -ne 'device') { throw 'no UNO Q found over adb (is it plugged in? see `adb devices`)' }

if ($RestoreArduinoLoader) {
    Write-Host '==> Flashing Arduino''s sketch loader back onto the STM32' -ForegroundColor Cyan
    Invoke-Adb 'push flash.cfg' push $flashCfg /tmp/notes-flash.cfg
    Invoke-Adb 'flash loader' shell "$remoteocd upload -f /tmp/notes-flash.cfg $arduinoLoader"
    Write-Host 'Done: Arduino sketches from App Lab / arduino-cli run again. (notes-server keeps' -ForegroundColor Green
    Write-Host 'running; its LED matrix status just stops until notes-matrix is flashed again.)' -ForegroundColor Green
    return
}

if (-not $NoBuild) {
    & (Join-Path $PSScriptRoot 'build-unoq.ps1') -ServerOnly:$NoFirmware
    if (-not $?) { throw 'build failed' }
}

if ($Setup) {
    Write-Host '==> One-time board setup (you will be asked for the board''s sudo password)' -ForegroundColor Cyan
    Invoke-Adb 'mkdir' shell 'rm -rf /tmp/notes-setup && mkdir -p /tmp/notes-setup'
    foreach ($f in 'setup-unoq.sh', 'notes-server.service', 'notes-server.env.example', 'certbot-deploy-hook.sh') {
        Invoke-Adb "push $f" push (Join-Path $PSScriptRoot $f) /tmp/notes-setup/
    }
    # -t gives sudo a terminal to ask for the password on.
    Invoke-Adb 'setup-unoq.sh' shell -t 'sudo bash /tmp/notes-setup/setup-unoq.sh && rm -rf /tmp/notes-setup'
}

if (-not $NoFirmware) {
    if (-not (Test-Path $firmware)) { throw "$firmware not found; run deploy\build-unoq.ps1" }
    Write-Host '==> Flashing the LED matrix firmware (Zephyr) onto the STM32' -ForegroundColor Cyan
    Invoke-Adb 'push firmware' push $firmware /tmp/notes-matrix.elf
    Invoke-Adb 'push flash.cfg' push $flashCfg /tmp/notes-flash.cfg
    Invoke-Adb 'flash firmware' shell "$remoteocd upload -f /tmp/notes-flash.cfg /tmp/notes-matrix.elf 2>&1 | grep -E 'Error|error|shutdown' ; rm -f /tmp/notes-matrix.elf /tmp/notes-flash.cfg"
}

if (-not (Test-Path $bin)) { throw "$bin not found; run deploy\build-unoq.ps1" }
Write-Host '==> Installing notes-server' -ForegroundColor Cyan
Invoke-Adb 'push binary' push $bin /tmp/notes-server.new
# /opt/notes-server is owned by the board's `arduino` user (set up by setup-unoq.sh), and
# sudoers allows restarting just this service without a password.
$remote = 'test -w /opt/notes-server || { echo "run deploy-unoq.ps1 -Setup first" >&2; exit 1; }' +
    ' && install -m 755 /tmp/notes-server.new /opt/notes-server/notes-server && rm -f /tmp/notes-server.new' +
    ' && sudo -n systemctl restart notes-server' +
    ' && sleep 2 && systemctl is-active notes-server' +
    ' || { journalctl -u notes-server -n 30 --no-pager 2>/dev/null; exit 1; }'
Invoke-Adb 'install/restart' shell $remote

Write-Host 'Deployed. Logs:  adb shell journalctl -u notes-server -f' -ForegroundColor Green
Write-Host 'From this laptop over USB:  adb forward tcp:8443 tcp:443  then  https://localhost:8443' -ForegroundColor Green
