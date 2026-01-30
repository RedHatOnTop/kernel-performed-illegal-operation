# KPIO 로컬 개발 환경 설정 가이드

이 문서는 KPIO 커널의 로컬 개발 및 테스트 환경 설정 방법을 설명합니다.

## 필수 요구사항

### 1. Rust 툴체인

```powershell
# rustup 설치 (없는 경우)
# https://rustup.rs 에서 rustup-init.exe 다운로드

# nightly 툴체인 설치
rustup toolchain install nightly-2026-01-01

# 필수 컴포넌트 설치
rustup component add rust-src --toolchain nightly-2026-01-01
rustup component add llvm-tools --toolchain nightly-2026-01-01
```

### 2. QEMU (x86_64 에뮬레이터)

```powershell
# winget으로 설치 (권장)
winget install SoftwareFreedomConservancy.QEMU

# 또는 Chocolatey
choco install qemu

# 또는 Scoop
scoop install qemu
```

설치 후 PATH에 추가되었는지 확인:
```powershell
qemu-system-x86_64 --version
```

### 3. OVMF (UEFI 펌웨어)

QEMU 설치 시 일반적으로 함께 설치됩니다. 다음 위치에서 확인:

- `C:\Program Files\qemu\share\edk2-x86_64-code.fd`
- `C:\Program Files\QEMU\share\OVMF.fd`

없는 경우 수동 다운로드:
```powershell
# 자동 다운로드 (setup-dev-env.ps1이 시도)
.\scripts\setup-dev-env.ps1

# 또는 수동 다운로드
# https://retrage.github.io/edk2-nightly/bin/RELEASEX64_OVMF.fd
# -> $HOME\.kpio\OVMF.fd 에 저장
```

## 자동 설정

모든 도구를 자동으로 확인하고 설치하려면:

```powershell
.\scripts\setup-dev-env.ps1
```

## 스크립트 목록

| 스크립트 | 설명 |
|---------|------|
| `setup-dev-env.ps1` | 개발 환경 설정 및 도구 설치 |
| `build-image.ps1` | 커널 빌드 + 디스크 이미지 생성 |
| `run-qemu.ps1` | QEMU에서 커널 실행 |
| `quick-run.ps1` | 빌드 + 즉시 실행 (간편 버전) |
| `run-tests.ps1` | 테스트 빌드 확인 |
| `create-uefi-image.ps1` | ESP 디렉토리 구조 생성 |

## 빠른 시작

### 1. 개발 환경 설정

```powershell
.\scripts\setup-dev-env.ps1
```

### 2. 커널 빌드

```powershell
cargo build -p kpio-kernel --release
```

### 3. 간편 실행

```powershell
.\scripts\quick-run.ps1
```

### 4. 전체 빌드 + 실행

```powershell
# 디스크 이미지 생성
.\scripts\build-image.ps1

# QEMU 실행
.\scripts\run-qemu.ps1
```

## 디버깅

### GDB 연결

```powershell
# 터미널 1: QEMU 디버그 모드 시작
.\scripts\run-qemu.ps1 -Debug

# 터미널 2: GDB 연결
gdb -ex "target remote :1234" -ex "symbol-file target/x86_64-unknown-none/release/kernel"
```

### 시리얼 출력 확인

QEMU는 `-serial stdio` 옵션으로 시리얼 출력을 터미널에 표시합니다.
커널의 `serial_println!` 매크로 출력이 여기에 나타납니다.

### 그래픽 없이 실행

```powershell
.\scripts\run-qemu.ps1 -NoGraphic
```

## 테스트

### 테스트 종료 코드

| 코드 | 의미 |
|------|------|
| 33 | 테스트 성공 (QemuExitCode::Success) |
| 35 | 테스트 실패 (QemuExitCode::Failed) |

### 테스트 실행

커널 테스트는 `#[cfg(test)]` 모드에서 QEMU를 통해 실행됩니다:

```powershell
# 테스트 빌드 확인
.\scripts\run-tests.ps1

# QEMU에서 테스트 실행
.\scripts\quick-run.ps1
```

## 문제 해결

### QEMU가 인식되지 않음

```powershell
# PATH에 QEMU 추가
$env:PATH = "C:\Program Files\qemu;$env:PATH"

# 영구 설정 (관리자 권한 필요)
[Environment]::SetEnvironmentVariable("PATH", "C:\Program Files\qemu;$([Environment]::GetEnvironmentVariable('PATH', 'Machine'))", "Machine")
```

### OVMF를 찾을 수 없음

```powershell
# OVMF 수동 다운로드
Invoke-WebRequest -Uri "https://retrage.github.io/edk2-nightly/bin/RELEASEX64_OVMF.fd" -OutFile "$HOME\.kpio\OVMF.fd"
```

### 커널이 부팅되지 않음

1. 시리얼 출력 확인 (`-serial stdio`)
2. QEMU 창의 디버그 콘솔 확인 (Ctrl+Alt+2)
3. GDB 연결하여 단계별 디버깅

### bootloader 빌드 실패

`tools/boot` 빌드가 실패하는 경우 `quick-run.ps1`을 사용하여 
QEMU의 직접 커널 로딩 기능으로 테스트할 수 있습니다:

```powershell
.\scripts\quick-run.ps1
```

## 구조

```
scripts/
├── setup-dev-env.ps1      # 개발 환경 설정
├── build-image.ps1        # 디스크 이미지 빌드
├── run-qemu.ps1           # QEMU 실행
├── quick-run.ps1          # 빌드 + 즉시 실행
├── run-tests.ps1          # 테스트 실행
└── create-uefi-image.ps1  # ESP 구조 생성

tools/boot/                # 부트로더 이미지 빌더
├── Cargo.toml
└── src/main.rs

target/
├── x86_64-unknown-none/release/
│   ├── kernel             # 커널 ELF
│   ├── kpio-uefi.img      # UEFI 디스크 이미지
│   └── kpio-bios.img      # BIOS 디스크 이미지
└── esp/                   # ESP 디렉토리 (FAT 가상)
    ├── EFI/BOOT/BOOTX64.EFI
    └── kernel
```
