<#
.SYNOPSIS
    Create a FAT32 test disk image for Phase 9-3 VFS verification.

.DESCRIPTION
    Creates `tests/e2e/test-disk.img` (16 MiB) and writes `HELLO.TXT` into it.
    Preferred path uses WSL tools (`mkfs.fat`, `mcopy`). If unavailable, prints
    setup instructions and exits with a non-zero code.
#>

param(
    [string]$OutputPath = "",
    [int]$SizeMiB = 16
)

$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $PSScriptRoot
if (-not $OutputPath) {
    $OutputPath = Join-Path $ProjectRoot "tests\e2e\test-disk.img"
}

$OutputDir = Split-Path -Parent $OutputPath
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

$TmpHello = Join-Path $env:TEMP "kpio-hello.txt"
"Hello from KPIO Phase 9-3 test disk." | Set-Content -Path $TmpHello -NoNewline -Encoding ASCII

$Wsl = Get-Command "wsl" -ErrorAction SilentlyContinue
if (-not $Wsl) {
    Write-Error "WSL is required to create FAT32 test disk automatically. Install WSL and dosfstools/mtools."
    exit 1
}

function Convert-ToWslPath([string]$WindowsPath) {
    $full = [System.IO.Path]::GetFullPath($WindowsPath)
     $drive = $full.Substring(0, 1).ToLowerInvariant()
    $rest = $full.Substring(2).Replace("\", "/")
     return "/mnt/$drive$rest"
}

$OutUnix = Convert-ToWslPath $OutputPath
$TmpUnix = Convert-ToWslPath $TmpHello

$cmd = @(
    "set -e",
    "truncate -s ${SizeMiB}M '$OutUnix'",
    "mkfs.fat -F 32 '$OutUnix'",
    "mcopy -i '$OutUnix' '$TmpUnix' ::HELLO.TXT"
) -join "; "

wsl bash -lc $cmd
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to create FAT32 image. Ensure WSL has dosfstools and mtools: sudo apt install dosfstools mtools"
    exit 1
}

Write-Host "[OK] Test disk created: $OutputPath"
Write-Host "      Contains: HELLO.TXT"
