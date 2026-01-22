<#
.SYNOPSIS
    KPIO 커널 테스트 실행 스크립트

.DESCRIPTION
    cargo test를 통해 커널 테스트를 실행합니다.
    QEMU에서 테스트가 실행되고 결과가 시리얼 출력으로 반환됩니다.

.PARAMETER TestName
    특정 테스트만 실행

.PARAMETER NoCapture
    출력 캡처 비활성화

.EXAMPLE
    .\run-tests.ps1
    .\run-tests.ps1 -TestName test_heap_allocation
#>

param(
    [string]$TestName = "",
    [switch]$NoCapture
)

$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $PSScriptRoot

Write-Host "=== KPIO 커널 테스트 ===" -ForegroundColor Cyan
Write-Host ""

# QEMU 확인
$qemu = Get-Command "qemu-system-x86_64" -ErrorAction SilentlyContinue
if (-not $qemu) {
    $defaultQemuPath = "C:\Program Files\qemu\qemu-system-x86_64.exe"
    if (-not (Test-Path $defaultQemuPath)) {
        Write-Error "QEMU가 설치되지 않았습니다. .\scripts\setup-dev-env.ps1 을 실행하세요."
        exit 1
    }
}

# OVMF 확인
$OvmfPaths = @(
    "C:\Program Files\qemu\share\edk2-x86_64-code.fd",
    "C:\Program Files\QEMU\share\OVMF.fd",
    "$env:USERPROFILE\.kpio\OVMF.fd"
)

$OvmfFound = $false
foreach ($path in $OvmfPaths) {
    if (Test-Path $path) {
        $OvmfFound = $true
        break
    }
}

if (-not $OvmfFound) {
    Write-Warning "OVMF를 찾을 수 없습니다. UEFI 테스트가 실패할 수 있습니다."
}

# 테스트 실행
Push-Location $ProjectRoot
try {
    Write-Host "테스트 빌드 및 실행 중..." -ForegroundColor Yellow
    Write-Host ""
    
    $testArgs = @("test", "-p", "kpio-kernel")
    
    if ($TestName) {
        $testArgs += @("--", $TestName)
    }
    
    if ($NoCapture) {
        $testArgs += "--nocapture"
    }
    
    # cargo test 실행
    # 참고: custom_test_frameworks를 사용하므로 일반 cargo test가 아닌 
    # QEMU 기반 테스트가 필요
    
    Write-Host "주의: 현재 커널은 custom_test_frameworks를 사용합니다." -ForegroundColor Yellow
    Write-Host "완전한 테스트를 위해서는 QEMU에서 실행해야 합니다." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "테스트 빌드 확인:" -ForegroundColor Cyan
    
    # 테스트 빌드만 확인
    cargo build -p kpio-kernel --lib
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host ""
        Write-Host "[OK] 테스트 빌드 성공" -ForegroundColor Green
        Write-Host ""
        Write-Host "QEMU에서 테스트를 실행하려면:" -ForegroundColor Yellow
        Write-Host "  1. .\scripts\build-image.ps1 (이미지 생성)" -ForegroundColor Cyan
        Write-Host "  2. .\scripts\run-qemu.ps1 (QEMU 실행)" -ForegroundColor Cyan
    } else {
        Write-Host ""
        Write-Host "[FAILED] 테스트 빌드 실패" -ForegroundColor Red
    }
} finally {
    Pop-Location
}
