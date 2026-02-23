<#
.SYNOPSIS
    KPIO 커널 간편 실행 스크립트

.DESCRIPTION
    커널을 빌드하고 QEMU에서 직접 실행합니다.
    bootloader 디스크 이미지 대신 QEMU의 직접 커널 로딩 기능을 사용합니다.
    
    주의: 이 방식은 완전한 UEFI 부팅이 아닌 multiboot 프로토콜을 사용합니다.
    완전한 UEFI pflash 테스트는 run-qemu.ps1을 사용하세요.
    
    BIOS 부팅은 bootloader 0.11.14 FAT 파서 오버플로 버그로 인해 권장되지 않습니다.
    자세한 내용은 docs/known-issues.md 를 참조하세요.

.PARAMETER Debug
    GDB 서버 활성화

.PARAMETER Release
    Release 빌드 사용 (기본값)

.PARAMETER NoGraphic
    그래픽 비활성화

.EXAMPLE
    .\quick-run.ps1
    .\quick-run.ps1 -Debug
#>

param(
    [switch]$Debug,
    [switch]$Release = $true,
    [switch]$NoGraphic
)

$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$KernelTarget = "x86_64-unknown-none"
$Profile = if ($Release) { "release" } else { "debug" }

Write-Host "=== KPIO 간편 실행 ===" -ForegroundColor Cyan
Write-Host ""

# 1. 커널 빌드
Write-Host "[1/2] 커널 빌드 중..." -ForegroundColor Yellow

Push-Location $ProjectRoot
try {
    $buildArgs = @("build", "-p", "kpio-kernel")
    if ($Release) {
        $buildArgs += "--release"
    }
    
    $proc = Start-Process -FilePath "cargo" -ArgumentList $buildArgs -NoNewWindow -Wait -PassThru
    if ($proc.ExitCode -ne 0) {
        throw "커널 빌드 실패"
    }
} finally {
    Pop-Location
}

$KernelPath = Join-Path $ProjectRoot "target\$KernelTarget\$Profile\kernel"
Write-Host "  커널 빌드 완료: $KernelPath" -ForegroundColor Green

# 2. QEMU 확인
Write-Host ""
Write-Host "[2/2] QEMU 실행 준비 중..." -ForegroundColor Yellow

$qemu = Get-Command "qemu-system-x86_64" -ErrorAction SilentlyContinue
if (-not $qemu) {
    # 기본 QEMU 설치 경로 확인
    $defaultQemuPath = "C:\Program Files\qemu\qemu-system-x86_64.exe"
    if (Test-Path $defaultQemuPath) {
        $qemu = $defaultQemuPath
    } else {
        Write-Error "QEMU가 설치되지 않았습니다."
        Write-Host ""
        Write-Host "설치 방법:" -ForegroundColor Yellow
        Write-Host "  winget install QEMU.QEMU" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "또는 .\scripts\setup-dev-env.ps1 실행" -ForegroundColor Cyan
        exit 1
    }
}

# OVMF 경로 찾기
$OvmfPaths = @(
    "C:\Program Files\qemu\share\edk2-x86_64-code.fd",
    "C:\Program Files\QEMU\share\OVMF.fd",
    "C:\Program Files\QEMU\share\edk2-x86_64-code.fd",
    "$env:USERPROFILE\scoop\apps\qemu\current\share\edk2-x86_64-code.fd",
    "$env:USERPROFILE\.kpio\OVMF.fd"
)

$OvmfPath = $null
foreach ($path in $OvmfPaths) {
    if (Test-Path $path) {
        $OvmfPath = $path
        break
    }
}

# ESP 디렉토리 생성 (QEMU fat: 드라이버용)
$EspDir = Join-Path $ProjectRoot "target\esp"
$EfiBootDir = Join-Path $EspDir "EFI\BOOT"

# UEFI 부트로더 EFI 찾기
$BootloaderOutDir = Join-Path $ProjectRoot "tools\boot\target"
$UefiEfi = Get-ChildItem $BootloaderOutDir -Recurse -Filter "bootloader-x86_64-uefi.efi" -ErrorAction SilentlyContinue | Select-Object -First 1

if ($UefiEfi -and $OvmfPath) {
    # UEFI 부팅 모드
    Write-Host "  UEFI 모드 사용" -ForegroundColor Green
    
    # ESP 구조 생성
    New-Item -ItemType Directory -Force -Path $EfiBootDir | Out-Null
    Copy-Item -Path $UefiEfi.FullName -Destination (Join-Path $EfiBootDir "BOOTX64.EFI") -Force
    Copy-Item -Path $KernelPath -Destination (Join-Path $EspDir "kernel") -Force
    
    # QEMU 인자 구성
    $QemuArgs = @(
        "-machine", "q35",
        "-cpu", "qemu64,+rdrand",
        "-m", "512M",
        "-serial", "stdio",
        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-netdev", "user,id=net0",
        "-device", "virtio-net-pci,netdev=net0",
        "-drive", "if=pflash,format=raw,readonly=on,file=$OvmfPath",
        "-drive", "format=raw,file=fat:rw:$EspDir"
    )
} else {
    # 직접 커널 로딩 모드 (multiboot)
    Write-Host "  직접 커널 로딩 모드 (UEFI 아님)" -ForegroundColor Yellow
    Write-Host "  주의: bootloader 통합 테스트가 아닙니다." -ForegroundColor Yellow
    
    $QemuArgs = @(
        "-machine", "q35",
        "-cpu", "qemu64,+rdrand",
        "-m", "512M",
        "-serial", "stdio",
        "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
        "-netdev", "user,id=net0",
        "-device", "virtio-net-pci,netdev=net0",
        "-kernel", $KernelPath
    )
}

if ($NoGraphic) {
    $QemuArgs += "-nographic"
} else {
    $QemuArgs += @("-vga", "std")
}

if ($Debug) {
    $QemuArgs += @("-s", "-S")
    Write-Host ""
    Write-Host "GDB 서버 시작됨 (포트 1234)" -ForegroundColor Cyan
    Write-Host "다른 터미널에서:" -ForegroundColor Gray
    Write-Host "  gdb -ex 'target remote :1234'" -ForegroundColor Cyan
    Write-Host ""
}

Write-Host ""
Write-Host "QEMU 실행 중..." -ForegroundColor Green
Write-Host "종료: Ctrl+A, X (또는 창 닫기)" -ForegroundColor Gray
Write-Host ""

# QEMU 실행
if ($qemu -is [string]) {
    & $qemu @QemuArgs
} else {
    & qemu-system-x86_64 @QemuArgs
}

# 종료 코드 확인
$ExitCode = $LASTEXITCODE
if ($ExitCode -eq 33) {
    Write-Host ""
    Write-Host "[SUCCESS] 테스트 통과 (exit code: 33)" -ForegroundColor Green
    exit 0
} elseif ($ExitCode -eq 35) {
    Write-Host ""
    Write-Host "[FAILED] 테스트 실패 (exit code: 35)" -ForegroundColor Red
    exit 1
} else {
    Write-Host ""
    Write-Host "[INFO] QEMU 종료 코드: $ExitCode" -ForegroundColor Yellow
    exit $ExitCode
}
