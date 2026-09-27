<#
.SYNOPSIS
  Cross-compiles notes-server for the Raspberry Pi 5 (64-bit Raspberry Pi OS) from Windows.

.DESCRIPTION
  Uses cargo-zigbuild: zig acts as the C cross-compiler and linker for the few C
  parts of the dependency tree (bundled SQLite, ring). The ".2.36" suffix links
  against glibc 2.36 (Raspberry Pi OS Bookworm), so the binary also runs on newer releases.

  Output: target\aarch64-unknown-linux-gnu\release\notes-server
          target\aarch64-unknown-linux-gnu\release\examples\bench_resize  (unless -SkipBench)
#>
param([switch]$SkipBench)

# Native tools (cargo, ssh) write progress to stderr; PowerShell 5.1 turns that into
# errors under 'Stop' when output is redirected. Failures are detected via $LASTEXITCODE.
$ErrorActionPreference = 'Continue'
$target = 'aarch64-unknown-linux-gnu'
$zigTarget = "$target.2.36"

Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    foreach ($tool in 'zig', 'cargo-zigbuild') {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            throw "$tool not found on PATH. One-time setup: winget install zig.zig; cargo install --locked cargo-zigbuild"
        }
    }
    if (-not (rustup target list --installed | Select-String -SimpleMatch -Quiet $target)) {
        rustup target add $target
        if ($LASTEXITCODE) { throw "rustup target add $target failed" }
    }

    cargo zigbuild --release --target $zigTarget --bin notes-server
    if ($LASTEXITCODE) { throw 'building notes-server failed' }

    if (-not $SkipBench) {
        cargo zigbuild --release --target $zigTarget --example bench_resize
        if ($LASTEXITCODE) { throw 'building bench_resize failed' }
    }

    $bin = Get-Item "target\$target\release\notes-server"
    Write-Host ("Built {0} ({1:N1} MB)" -f $bin.FullName, ($bin.Length / 1MB)) -ForegroundColor Green
}
finally {
    Pop-Location
}
