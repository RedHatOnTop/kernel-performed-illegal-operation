# KPIO 디스크 이미지 빌드 스크립트
# bootloader 크레이트를 사용하여 UEFI/BIOS 부팅 가능한 디스크 이미지 생성
#
# 사용법:
#   .\build-image.ps1                 # release 빌드
#   .\build-image.ps1 -Profile debug  # debug 빌드

param(
    [string]$Profile = "release",
    [switch]$BiosOnly,
    [switch]$UefiOnly
)

$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$TargetDir = Join-Path $ProjectRoot "target"
$KernelTarget = "x86_64-unknown-none"
$ToolsBootDir = Join-Path $ProjectRoot "tools\boot"

Write-Host "=== KPIO 디스크 이미지 빌더 ===" -ForegroundColor Cyan
Write-Host ""

# 1. 커널 빌드
Write-Host "[1/3] 커널 빌드 중..." -ForegroundColor Yellow

$KernelBuildArgs = @(
    "build",
    "-p", "kpio-kernel",
    "--target", $KernelTarget
)

if ($Profile -eq "release") {
    $KernelBuildArgs += "--release"
}

Push-Location $ProjectRoot
try {
    $proc = Start-Process -FilePath "cargo" -ArgumentList $KernelBuildArgs -NoNewWindow -Wait -PassThru
    if ($proc.ExitCode -ne 0) {
        throw "커널 빌드 실패"
    }
} finally {
    Pop-Location
}

$ProfileDir = if ($Profile -eq "release") { "release" } else { "debug" }
$KernelPath = Join-Path $TargetDir $KernelTarget $ProfileDir "kernel"

if (-not (Test-Path $KernelPath)) {
    throw "커널 바이너리를 찾을 수 없습니다: $KernelPath"
}

Write-Host "  커널 빌드 완료: $KernelPath" -ForegroundColor Green
Write-Host ""

# 2. 부트 이미지 빌더 확인 (이미 빌드된 경우 재사용)
Write-Host "[2/3] 이미지 빌더 확인 중..." -ForegroundColor Yellow

if (-not (Test-Path $ToolsBootDir)) {
    throw "tools/boot 디렉토리를 찾을 수 없습니다: $ToolsBootDir"
}

$ImageBuilder = Join-Path $ToolsBootDir "target\x86_64-pc-windows-msvc\release\build-image.exe"
if (-not (Test-Path $ImageBuilder)) {
    # fallback 경로
    $ImageBuilder = Join-Path $ToolsBootDir "target\release\build-image.exe"
}

if (-not (Test-Path $ImageBuilder)) {
    Write-Host "  이미지 빌더가 없습니다. 빌드를 시도합니다..." -ForegroundColor Yellow
    Write-Host "  주의: 상위 workspace와 격리된 환경에서 빌드해야 합니다." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  수동 빌드 방법:" -ForegroundColor Cyan
    Write-Host '    $tempDir = "$env:TEMP\kpio-boot-builder"' -ForegroundColor Gray
    Write-Host '    Copy-Item -Recurse tools\boot\* $tempDir' -ForegroundColor Gray
    Write-Host '    Remove-Item "$tempDir\.cargo" -Recurse -Force' -ForegroundColor Gray
    Write-Host '    cd $tempDir' -ForegroundColor Gray
    Write-Host '    cargo +nightly build --release --target x86_64-pc-windows-msvc' -ForegroundColor Gray
    Write-Host '    Copy-Item -Recurse target\* tools\boot\target' -ForegroundColor Gray
    Write-Host ""
    throw "이미지 빌더가 없습니다. 위 명령으로 수동 빌드 후 다시 시도하세요."
}


if (-not (Test-Path $ImageBuilder)) {
    throw "이미지 빌더를 찾을 수 없습니다: $ImageBuilder"
}

Write-Host "  이미지 빌더 빌드 완료" -ForegroundColor Green
Write-Host ""

# 3. 디스크 이미지 생성
Write-Host "[3/3] 디스크 이미지 생성 중..." -ForegroundColor Yellow

$proc = Start-Process -FilePath $ImageBuilder -ArgumentList $KernelPath -NoNewWindow -Wait -PassThru
if ($proc.ExitCode -ne 0) {
    throw "디스크 이미지 생성 실패"
}

Write-Host ""
Write-Host "=== 완료 ===" -ForegroundColor Green
Write-Host ""

$UefiImagePath = Join-Path $TargetDir $KernelTarget $ProfileDir "kpio-uefi.img"
$BiosImagePath = Join-Path $TargetDir $KernelTarget $ProfileDir "kpio-bios.img"

if (Test-Path $UefiImagePath) {
    Write-Host "UEFI 이미지: $UefiImagePath" -ForegroundColor Cyan
}
if (Test-Path $BiosImagePath) {
    Write-Host "BIOS 이미지: $BiosImagePath" -ForegroundColor Cyan
}

Write-Host ""
Write-Host "QEMU로 실행하려면:" -ForegroundColor Yellow
Write-Host "  .\scripts\run-qemu.ps1"
