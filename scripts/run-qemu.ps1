<# 
.SYNOPSIS
    KPIO 커널 QEMU 실행 스크립트

.DESCRIPTION
    빌드된 커널 이미지를 QEMU에서 실행합니다.
    UEFI 부팅을 지원하며 디버깅 옵션을 제공합니다.

.PARAMETER Debug
    GDB 서버를 활성화하고 시작 시 일시정지합니다.

.PARAMETER NoGraphic
    그래픽 출력 없이 시리얼 콘솔만 사용합니다.

.PARAMETER Memory
    메모리 크기 (기본값: 512M)

.PARAMETER Bios
    BIOS 모드로 부팅 (기본값: UEFI)

.EXAMPLE
    .\run-qemu.ps1
    .\run-qemu.ps1 -Debug
    .\run-qemu.ps1 -NoGraphic -Memory 1G
#>

param(
    [switch]$Debug,
    [switch]$NoGraphic,
    [switch]$Bios,
    [string]$Memory = "512M"
)

$ErrorActionPreference = "Stop"

# 프로젝트 루트 디렉토리
$ProjectRoot = Split-Path -Parent $PSScriptRoot

# 기본 이미지 경로 (release 빌드)
$KernelTarget = "x86_64-unknown-none"
$TargetDir = Join-Path $ProjectRoot "target\$KernelTarget\release"

# QEMU 경로 확인
$QemuPath = Get-Command "qemu-system-x86_64" -ErrorAction SilentlyContinue
if (-not $QemuPath) {
    Write-Error "QEMU가 설치되지 않았습니다. 'winget install QEMU.QEMU' 로 설치하세요."
    exit 1
}

# 이미지 경로
$UefiImage = Join-Path $TargetDir "kpio-uefi.img"
$BiosImage = Join-Path $TargetDir "kpio-bios.img"

# 이미지 존재 확인
if ($Bios) {
    $DiskImage = $BiosImage
    if (-not (Test-Path $DiskImage)) {
        Write-Error "BIOS 이미지를 찾을 수 없습니다: $DiskImage"
        Write-Host "먼저 '.\scripts\build-image.ps1'로 이미지를 빌드하세요."
        exit 1
    }
} else {
    $DiskImage = $UefiImage
    if (-not (Test-Path $DiskImage)) {
        Write-Error "UEFI 이미지를 찾을 수 없습니다: $DiskImage"
        Write-Host "먼저 '.\scripts\build-image.ps1'로 이미지를 빌드하세요."
        exit 1
    }
}

# OVMF 경로 찾기 (UEFI 펌웨어)
$OvmfPaths = @(
    "$env:USERPROFILE\.kpio\OVMF.fd",
    "C:\Program Files\qemu\share\edk2-x86_64-code.fd",
    "C:\Program Files\QEMU\share\OVMF.fd",
    "C:\Program Files\qemu\share\OVMF.fd",
    "$env:USERPROFILE\scoop\apps\qemu\current\share\edk2-x86_64-code.fd"
)

$OvmfPath = $null
foreach ($path in $OvmfPaths) {
    if (Test-Path $path) {
        $OvmfPath = $path
        break
    }
}

# QEMU 인자 구성
$QemuArgs = @(
    "-machine", "q35",
    "-cpu", "qemu64,+rdrand",
    "-m", $Memory,
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"
)

# 그래픽 옵션
if ($NoGraphic) {
    $QemuArgs += "-nographic"
    # Windows에서 -serial stdio 대신 -serial mon:stdio 사용
    $QemuArgs += @("-serial", "mon:stdio")
} else {
    $QemuArgs += @("-vga", "std")
    # 그래픽 모드에서는 시리얼을 파일로 출력
    $SerialLog = Join-Path $TargetDir "serial.log"
    $QemuArgs += @("-serial", "file:$SerialLog")
    Write-Host "시리얼 출력: $SerialLog"
}

# 디버그 옵션
if ($Debug) {
    $QemuArgs += @("-s", "-S")
    Write-Host "GDB 서버 시작됨 (포트 1234)"
    Write-Host "다른 터미널에서 다음 명령으로 연결하세요:"
    Write-Host "  gdb -ex 'target remote :1234' -ex 'symbol-file target/x86_64-kpio/release/kpio-kernel'"
    Write-Host ""
}

# 부팅 모드
if ($Bios) {
    $QemuArgs += @(
        "-drive", "format=raw,file=$DiskImage"
    )
    Write-Host "BIOS 모드로 부팅합니다..."
} else {
    if (-not $OvmfPath) {
        Write-Error "OVMF(UEFI 펌웨어)를 찾을 수 없습니다."
        Write-Host "다음 위치 중 하나에 OVMF.fd를 설치하세요:"
        $OvmfPaths | ForEach-Object { Write-Host "  - $_" }
        exit 1
    }
    
    # QEMU 10.x requires pflash for UEFI firmware (not -bios)
    $QemuArgs += @(
        "-drive", "if=pflash,format=raw,readonly=on,file=$OvmfPath",
        "-drive", "format=raw,file=$DiskImage"
    )
    Write-Host "UEFI 모드로 부팅합니다... (pflash)"
}

Write-Host "디스크 이미지: $DiskImage"
Write-Host "메모리: $Memory"
Write-Host ""

# QEMU 실행
& qemu-system-x86_64 @QemuArgs

# 종료 코드 확인
$ExitCode = $LASTEXITCODE
if ($ExitCode -eq 33) {
    Write-Host "`n[SUCCESS] 테스트 통과" -ForegroundColor Green
    exit 0
} elseif ($ExitCode -eq 35) {
    Write-Host "`n[FAILED] 테스트 실패" -ForegroundColor Red
    exit 1
} else {
    Write-Host "`n[INFO] QEMU 종료 코드: $ExitCode"
    exit $ExitCode
}
