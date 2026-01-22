# KPIO UEFI 디스크 이미지 빌드 스크립트
# bootloader 크레이트가 제공하는 UEFI EFI 파일과 커널을 결합하여 FAT 이미지 생성
#
# 사전 요구사항:
# - tools/boot에서 cargo +nightly build --release 실행하여 bootloader EFI 생성
# - 커널 빌드 완료
#
# 사용법:
#   .\create-uefi-image.ps1

param(
    [string]$Profile = "release"
)

$ErrorActionPreference = "Stop"

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$KernelTarget = "x86_64-unknown-none"
$ProfileDir = if ($Profile -eq "release") { "release" } else { "debug" }

# 경로 설정
$KernelPath = Join-Path $ProjectRoot "target\$KernelTarget\$ProfileDir\kpio-kernel"
$BootloaderOutDir = Join-Path $ProjectRoot "tools\boot\target\x86_64-pc-windows-msvc\release\build"
$OutputDir = Join-Path $ProjectRoot "target\$KernelTarget\$ProfileDir"

Write-Host "=== KPIO UEFI 이미지 빌더 ===" -ForegroundColor Cyan
Write-Host ""

# 1. 커널 바이너리 확인
if (-not (Test-Path $KernelPath)) {
    Write-Error "커널 바이너리를 찾을 수 없습니다: $KernelPath"
    Write-Host "먼저 다음 명령으로 커널을 빌드하세요:"
    Write-Host "  cargo build -p kpio-kernel --release"
    exit 1
}
Write-Host "커널: $KernelPath" -ForegroundColor Green

# 2. UEFI 부트로더 EFI 파일 찾기
$UefiEfi = Get-ChildItem $BootloaderOutDir -Recurse -Filter "bootloader-x86_64-uefi.efi" -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $UefiEfi) {
    Write-Error "UEFI 부트로더 EFI 파일을 찾을 수 없습니다."
    Write-Host "먼저 다음 명령으로 부트로더를 빌드하세요:"
    Write-Host "  cd tools\boot"
    Write-Host "  cargo +nightly build --release"
    exit 1
}
Write-Host "UEFI 부트로더: $($UefiEfi.FullName)" -ForegroundColor Green

# 3. FAT 이미지 생성을 위한 도구 확인
# mtools가 필요하거나, PowerShell로 FAT 이미지 생성

# 간단한 방법: ESP 구조 폴더 생성 후 qemu-img로 변환
$EspDir = Join-Path $ProjectRoot "target\esp"
$EfiBootDir = Join-Path $EspDir "EFI\BOOT"

# ESP 디렉토리 구조 생성
New-Item -ItemType Directory -Force -Path $EfiBootDir | Out-Null

# UEFI 부트로더를 BOOTX64.EFI로 복사
Copy-Item -Path $UefiEfi.FullName -Destination (Join-Path $EfiBootDir "BOOTX64.EFI") -Force

# 커널을 ESP 루트에 복사 (bootloader가 찾을 수 있도록)
Copy-Item -Path $KernelPath -Destination (Join-Path $EspDir "kernel") -Force

Write-Host ""
Write-Host "ESP 디렉토리 구조 생성됨: $EspDir" -ForegroundColor Green
Write-Host ""

# 4. 참고 사항 출력
Write-Host "=== QEMU 실행 방법 ===" -ForegroundColor Yellow
Write-Host ""
Write-Host "QEMU에서 UEFI 부팅을 위해 다음과 같이 실행하세요:"
Write-Host ""
Write-Host '  $OvmfPath = "C:\Program Files\qemu\share\edk2-x86_64-code.fd"' -ForegroundColor Cyan
Write-Host '  qemu-system-x86_64 `' -ForegroundColor Cyan
Write-Host '    -machine q35 `' -ForegroundColor Cyan  
Write-Host '    -cpu qemu64 `' -ForegroundColor Cyan
Write-Host '    -m 512M `' -ForegroundColor Cyan
Write-Host '    -serial stdio `' -ForegroundColor Cyan
Write-Host '    -bios $OvmfPath `' -ForegroundColor Cyan
Write-Host "    -drive format=raw,file=fat:rw:$EspDir" -ForegroundColor Cyan
Write-Host ""
Write-Host "또는 FAT 이미지 파일을 생성하려면:"
Write-Host ""
Write-Host "  # mtools 설치 후:" -ForegroundColor Gray
Write-Host '  mformat -C -F -i target\uefi.img ::' -ForegroundColor Gray
Write-Host '  mcopy -i target\uefi.img -s target\esp\* ::/' -ForegroundColor Gray
