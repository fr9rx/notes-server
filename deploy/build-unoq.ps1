<#
.SYNOPSIS
  Builds everything for the Arduino UNO Q on this Windows laptop.

.DESCRIPTION
  1. The React website (frontend/, embedded in the server binary), then
     notes-server for the board's Linux side (QRB2210, aarch64 Cortex-A53, Debian),
     cross-compiled with cargo-zigbuild. zig is only the C cross-compiler/linker for
     the small C parts of the dependency tree (bundled SQLite, ring). ".2.36" links
     against glibc 2.36, so the binary runs on the board's Debian 13 (glibc 2.41).
  2. notes-matrix, the native Zephyr firmware for the board's STM32U585 (LED matrix),
     with west + the Zephyr SDK from the mcu\ workspace (see README "Laptop setup").

  Output: target\aarch64-unknown-linux-gnu\release\notes-server
          mcu\build\zephyr\zephyr.elf
#>
param(
    [switch]$ServerOnly,
    [switch]$FirmwareOnly
)

# Native tools write progress to stderr; PowerShell 5.1 turns that into errors
# under 'Stop' when output is redirected. Failures are detected via $LASTEXITCODE.
$ErrorActionPreference = 'Continue'
$root = Split-Path -Parent $PSScriptRoot
$target = 'aarch64-unknown-linux-gnu'

Push-Location $root
try {
    if (-not $FirmwareOnly) {
        # The website is embedded in the server binary, so build it first.
        if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
            throw 'npm not found on PATH. Install Node.js LTS (see README "Laptop setup").'
        }
        Push-Location (Join-Path $root 'frontend')
        try {
            if (-not (Test-Path 'node_modules')) {
                npm ci --no-audit --no-fund
                if ($LASTEXITCODE) { throw 'npm ci failed' }
            }
            npm run build
            if ($LASTEXITCODE) { throw 'building the frontend failed' }
        }
        finally {
            Pop-Location
        }

        foreach ($tool in 'zig', 'cargo-zigbuild') {
            if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
                throw "$tool not found on PATH. One-time setup: winget install zig.zig; cargo install --locked cargo-zigbuild"
            }
        }
        if (-not (rustup target list --installed | Select-String -SimpleMatch -Quiet $target)) {
            rustup target add $target
            if ($LASTEXITCODE) { throw "rustup target add $target failed" }
        }
        cargo zigbuild --release --target "$target.2.36" --bin notes-server
        if ($LASTEXITCODE) { throw 'building notes-server failed' }
        $bin = Get-Item "target\$target\release\notes-server"
        Write-Host ("Built {0} ({1:N1} MB)" -f $bin.FullName, ($bin.Length / 1MB)) -ForegroundColor Green
    }

    if (-not $ServerOnly) {
        $west = Join-Path $root 'mcu\.venv\Scripts\west.exe'
        if (-not (Test-Path $west)) {
            throw "Zephyr workspace not set up (mcu\.venv missing). See README 'Laptop setup'."
        }
        Push-Location (Join-Path $root 'mcu')
        try {
            & $west build -b arduino_uno_q notes-matrix -d build
            if ($LASTEXITCODE) { throw 'building the notes-matrix firmware failed' }
        }
        finally {
            Pop-Location
        }
        Write-Host "Built mcu\build\zephyr\zephyr.elf" -ForegroundColor Green
    }
}
finally {
    Pop-Location
}
