<#
.SYNOPSIS
    KPIO 개발 환경 설정 스크립트

.DESCRIPTION
    로컬 커널 개발 및 테스트에 필요한 도구들을 설치합니다.
    - QEMU (x86_64 에뮬레이터)
    - OVMF (UEFI 펌웨어)
    - 선택적: GDB, llvm-tools

.EXAMPLE
    .\setup-dev-env.ps1
    .\setup-dev-env.ps1 -SkipQemu
#>

param(
    [switch]$SkipQemu,
    [switch]$SkipRustTools,
    [switch]$Force
)

$ErrorActionPreference = "Stop"

Write-Host "=== KPIO 개발 환경 설정 ===" -ForegroundColor Cyan
Write-Host ""

# 관리자 권한 확인
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

# 1. QEMU 설치 확인 및 설치
if (-not $SkipQemu) {
    Write-Host "[1/4] QEMU 확인 중..." -ForegroundColor Yellow
    
    $qemuPath = Get-Command "qemu-system-x86_64" -ErrorAction SilentlyContinue
    
    if ($qemuPath -and -not $Force) {
        Write-Host "  QEMU가 이미 설치되어 있습니다: $($qemuPath.Source)" -ForegroundColor Green
    } else {
        Write-Host "  QEMU가 설치되지 않았습니다. 설치를 시도합니다..." -ForegroundColor Yellow
        
        # winget 확인
        $winget = Get-Command "winget" -ErrorAction SilentlyContinue
        if ($winget) {
            Write-Host "  winget으로 QEMU 설치 중..." -ForegroundColor Cyan
            winget install SoftwareFreedomConservancy.QEMU --accept-package-agreements --accept-source-agreements
            
            # PATH 갱신
            $qemuInstallPath = "C:\Program Files\qemu"
            if (Test-Path $qemuInstallPath) {
                $env:PATH = "$qemuInstallPath;$env:PATH"
                Write-Host "  QEMU 설치 완료. PATH에 추가됨: $qemuInstallPath" -ForegroundColor Green
                Write-Host "  주의: 현재 터미널에서만 적용됩니다. 새 터미널에서는 시스템 환경 변수를 확인하세요." -ForegroundColor Yellow
            }
        } else {
            Write-Host ""
            Write-Host "  winget이 없습니다. 수동으로 QEMU를 설치하세요:" -ForegroundColor Red
            Write-Host "    1. https://www.qemu.org/download/#windows 에서 다운로드"
            Write-Host "    2. 또는: choco install qemu"
            Write-Host "    3. 또는: scoop install qemu"
            Write-Host ""
        }
    }
} else {
    Write-Host "[1/4] QEMU 설치 건너뜀" -ForegroundColor Gray
}

# 2. OVMF (UEFI 펌웨어) 확인
Write-Host ""
Write-Host "[2/4] OVMF (UEFI 펌웨어) 확인 중..." -ForegroundColor Yellow

$OvmfPaths = @(
    "C:\Program Files\qemu\share\edk2-x86_64-code.fd",
    "C:\Program Files\QEMU\share\OVMF.fd",
    "C:\Program Files\QEMU\share\edk2-x86_64-code.fd",
    "$env:USERPROFILE\scoop\apps\qemu\current\share\edk2-x86_64-code.fd",
    "$env:USERPROFILE\.kpio\OVMF.fd"
)

$OvmfFound = $false
foreach ($path in $OvmfPaths) {
    if (Test-Path $path) {
        Write-Host "  OVMF 발견: $path" -ForegroundColor Green
        $OvmfFound = $true
        break
    }
}

if (-not $OvmfFound) {
    Write-Host "  OVMF를 찾을 수 없습니다." -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  QEMU 설치 시 일반적으로 함께 설치됩니다." -ForegroundColor Gray
    Write-Host "  수동 다운로드: https://www.kraxel.org/repos/jenkins/edk2/" -ForegroundColor Gray
    Write-Host ""
    
    # OVMF 자동 다운로드 시도
    $ovmfDownloadDir = "$env:USERPROFILE\.kpio"
    $ovmfTargetPath = "$ovmfDownloadDir\OVMF.fd"
    
    if (-not (Test-Path $ovmfDownloadDir)) {
        New-Item -ItemType Directory -Force -Path $ovmfDownloadDir | Out-Null
    }
    
    Write-Host "  OVMF 다운로드를 시도합니다..." -ForegroundColor Cyan
    
    try {
        # RPM 대신 직접 바이너리 사용 가능한 소스
        $ovmfUrl = "https://retrage.github.io/edk2-nightly/bin/RELEASEX64_OVMF.fd"
        Invoke-WebRequest -Uri $ovmfUrl -OutFile $ovmfTargetPath -UseBasicParsing
        Write-Host "  OVMF 다운로드 완료: $ovmfTargetPath" -ForegroundColor Green
    } catch {
        Write-Host "  OVMF 자동 다운로드 실패. 수동으로 다운로드하세요." -ForegroundColor Red
    }
}

# 3. Rust 도구 확인
if (-not $SkipRustTools) {
    Write-Host ""
    Write-Host "[3/4] Rust 도구 확인 중..." -ForegroundColor Yellow
    
    # rustup 확인
    $rustup = Get-Command "rustup" -ErrorAction SilentlyContinue
    if (-not $rustup) {
        Write-Host "  rustup이 설치되지 않았습니다." -ForegroundColor Red
        Write-Host "  https://rustup.rs 에서 설치하세요." -ForegroundColor Yellow
    } else {
        # nightly 툴체인 확인
        $toolchains = rustup toolchain list 2>&1
        if ($toolchains -match "nightly") {
            Write-Host "  Nightly 툴체인 설치됨" -ForegroundColor Green
        } else {
            Write-Host "  Nightly 툴체인 설치 중..." -ForegroundColor Cyan
            rustup toolchain install nightly
        }
        
        # rust-src 컴포넌트 확인
        $components = rustup component list --toolchain nightly 2>&1
        if ($components -match "rust-src.*installed") {
            Write-Host "  rust-src 컴포넌트 설치됨" -ForegroundColor Green
        } else {
            Write-Host "  rust-src 컴포넌트 설치 중..." -ForegroundColor Cyan
            rustup component add rust-src --toolchain nightly
        }
        
        # llvm-tools 확인
        if ($components -match "llvm-tools.*installed") {
            Write-Host "  llvm-tools 컴포넌트 설치됨" -ForegroundColor Green
        } else {
            Write-Host "  llvm-tools 컴포넌트 설치 중..." -ForegroundColor Cyan
            rustup component add llvm-tools --toolchain nightly
        }
    }
} else {
    Write-Host "[3/4] Rust 도구 확인 건너뜀" -ForegroundColor Gray
}

# 4. 요약
Write-Host ""
Write-Host "[4/4] 환경 확인 완료" -ForegroundColor Yellow
Write-Host ""
Write-Host "=== 설정 요약 ===" -ForegroundColor Cyan
Write-Host ""

# QEMU 상태
$qemuPath = Get-Command "qemu-system-x86_64" -ErrorAction SilentlyContinue
if ($qemuPath) {
    Write-Host "[OK] QEMU: $($qemuPath.Source)" -ForegroundColor Green
} else {
    Write-Host "[--] QEMU: 설치 필요" -ForegroundColor Red
}

# OVMF 상태
$OvmfFound = $false
foreach ($path in $OvmfPaths) {
    if (Test-Path $path) {
        Write-Host "[OK] OVMF: $path" -ForegroundColor Green
        $OvmfFound = $true
        break
    }
}
if (-not $OvmfFound) {
    Write-Host "[--] OVMF: 설치 필요" -ForegroundColor Red
}

# Rust 상태
$rustup = Get-Command "rustup" -ErrorAction SilentlyContinue
if ($rustup) {
    Write-Host "[OK] Rust toolchain" -ForegroundColor Green
} else {
    Write-Host "[--] Rust: 설치 필요" -ForegroundColor Red
}

Write-Host ""
Write-Host "=== 다음 단계 ===" -ForegroundColor Yellow
Write-Host ""
Write-Host "1. 커널 빌드:"
Write-Host "   cargo build -p kpio-kernel --release" -ForegroundColor Cyan
Write-Host ""
Write-Host "2. 디스크 이미지 생성:"
Write-Host "   .\scripts\build-image.ps1" -ForegroundColor Cyan
Write-Host ""
Write-Host "3. QEMU에서 실행:"
Write-Host "   .\scripts\run-qemu.ps1" -ForegroundColor Cyan
Write-Host ""
