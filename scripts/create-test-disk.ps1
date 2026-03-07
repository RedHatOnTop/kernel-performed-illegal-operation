<#
.SYNOPSIS
    Create a FAT32 test disk image for KPIO integration testing.

.DESCRIPTION
    Creates `tests/e2e/test-disk.img` (16 MiB) with a FAT32 filesystem
    containing:
      - HELLO.TXT   — text file for VFS read self-tests
      - INIT         — minimal ELF64 binary (prints "[INIT] PID 1 running")
      - BIN/HELLO    — minimal ELF64 binary (prints "Hello from disk!")
    Requires WSL with dosfstools and mtools.
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

$Wsl = Get-Command "wsl" -ErrorAction SilentlyContinue
if (-not $Wsl) {
    Write-Error "WSL is required to create FAT32 test disk automatically. Install WSL and dosfstools/mtools."
    exit 1
}

# ── Helper: build a minimal static ELF64 binary from raw x86_64 code ──

function Build-MinimalElf {
    param(
        [byte[]]$Code,
        [uint64]$VAddr = 0x400000
    )

    $elfHeaderSize = 64
    $phdrSize = 56
    $codeOffset = $elfHeaderSize + $phdrSize  # 120
    $totalSize = $codeOffset + $Code.Length
    $entryPoint = $VAddr + $codeOffset

    $elf = [byte[]]::new($totalSize)

    # ELF magic
    $elf[0] = 0x7F; $elf[1] = [byte][char]'E'; $elf[2] = [byte][char]'L'; $elf[3] = [byte][char]'F'
    $elf[4] = 2    # ELFCLASS64
    $elf[5] = 1    # ELFDATA2LSB
    $elf[6] = 1    # EV_CURRENT

    # e_type = ET_EXEC (2)
    [System.BitConverter]::GetBytes([uint16]2).CopyTo($elf, 16)
    # e_machine = EM_X86_64 (62)
    [System.BitConverter]::GetBytes([uint16]62).CopyTo($elf, 18)
    # e_version = 1
    [System.BitConverter]::GetBytes([uint32]1).CopyTo($elf, 20)
    # e_entry
    [System.BitConverter]::GetBytes([uint64]$entryPoint).CopyTo($elf, 24)
    # e_phoff = 64
    [System.BitConverter]::GetBytes([uint64]64).CopyTo($elf, 32)
    # e_ehsize = 64
    [System.BitConverter]::GetBytes([uint16]64).CopyTo($elf, 52)
    # e_phentsize = 56
    [System.BitConverter]::GetBytes([uint16]56).CopyTo($elf, 54)
    # e_phnum = 1
    [System.BitConverter]::GetBytes([uint16]1).CopyTo($elf, 56)

    # Program header (PT_LOAD) at offset 64
    $ph = 64
    # p_type = PT_LOAD (1)
    [System.BitConverter]::GetBytes([uint32]1).CopyTo($elf, $ph)
    # p_flags = PF_R | PF_X (5)
    [System.BitConverter]::GetBytes([uint32]5).CopyTo($elf, $ph + 4)
    # p_offset = 0
    [System.BitConverter]::GetBytes([uint64]0).CopyTo($elf, $ph + 8)
    # p_vaddr
    [System.BitConverter]::GetBytes([uint64]$VAddr).CopyTo($elf, $ph + 16)
    # p_paddr
    [System.BitConverter]::GetBytes([uint64]$VAddr).CopyTo($elf, $ph + 24)
    # p_filesz
    [System.BitConverter]::GetBytes([uint64]$totalSize).CopyTo($elf, $ph + 32)
    # p_memsz
    [System.BitConverter]::GetBytes([uint64]$totalSize).CopyTo($elf, $ph + 40)
    # p_align = 0x1000
    [System.BitConverter]::GetBytes([uint64]0x1000).CopyTo($elf, $ph + 48)

    # Copy code payload
    [System.Array]::Copy($Code, 0, $elf, $codeOffset, $Code.Length)

    return $elf
}

# ── Generate ELF binaries ──

# /INIT: prints "[INIT] PID 1 running\n" (21 bytes) then loops forever.
#   mov rax, 1           ; SYS_WRITE
#   mov rdi, 1           ; stdout
#   lea rsi, [rip+0x0B]  ; message (32 - 21 = 11 bytes ahead)
#   mov rdx, 21          ; length
#   syscall
#   jmp $                 ; infinite loop (scheduler preempts)
#   db "[INIT] PID 1 running", 0x0A
[byte[]]$initCode = @(
    0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00,   # mov rax, 1
    0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00,   # mov rdi, 1
    0x48, 0x8D, 0x35, 0x0B, 0x00, 0x00, 0x00,   # lea rsi, [rip+0x0B]
    0x48, 0xC7, 0xC2, 0x15, 0x00, 0x00, 0x00,   # mov rdx, 21
    0x0F, 0x05,                                   # syscall
    0xEB, 0xFE,                                   # jmp $ (loop)
    0x5B, 0x49, 0x4E, 0x49, 0x54, 0x5D, 0x20,   # [INIT]<space>
    0x50, 0x49, 0x44, 0x20, 0x31, 0x20,         # PID 1<space>
    0x72, 0x75, 0x6E, 0x6E, 0x69, 0x6E, 0x67,   # running
    0x0A                                          # \n
)

# /BIN/HELLO: prints "Hello from disk!\n" (17 bytes) then exits.
#   mov rax, 1           ; SYS_WRITE
#   mov rdi, 1           ; stdout
#   lea rsi, [rip+0x15]  ; message (42 - 21 = 21 bytes ahead)
#   mov rdx, 17          ; length
#   syscall
#   mov rax, 60          ; SYS_EXIT
#   xor rdi, rdi         ; exit code 0
#   syscall
#   db "Hello from disk!", 0x0A
[byte[]]$helloCode = @(
    0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00,   # mov rax, 1
    0x48, 0xC7, 0xC7, 0x01, 0x00, 0x00, 0x00,   # mov rdi, 1
    0x48, 0x8D, 0x35, 0x15, 0x00, 0x00, 0x00,   # lea rsi, [rip+0x15]
    0x48, 0xC7, 0xC2, 0x11, 0x00, 0x00, 0x00,   # mov rdx, 17
    0x0F, 0x05,                                   # syscall
    0x48, 0xC7, 0xC0, 0x3C, 0x00, 0x00, 0x00,   # mov rax, 60
    0x48, 0x31, 0xFF,                             # xor rdi, rdi
    0x0F, 0x05,                                   # syscall
    0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x20,         # Hello<space>
    0x66, 0x72, 0x6F, 0x6D, 0x20,               # from<space>
    0x64, 0x69, 0x73, 0x6B, 0x21,               # disk!
    0x0A                                          # \n
)

$initElf  = Build-MinimalElf -Code $initCode
$helloElf = Build-MinimalElf -Code $helloCode

# ── Write temporary files ──

$TmpDir   = Join-Path $env:TEMP "kpio-disk-staging"
if (-not (Test-Path $TmpDir)) { New-Item -ItemType Directory -Path $TmpDir | Out-Null }

$TmpHello    = Join-Path $TmpDir "HELLO.TXT"
$TmpInit     = Join-Path $TmpDir "INIT"
$TmpHelloElf = Join-Path $TmpDir "HELLO"

"Hello from KPIO Phase 9-3 test disk." | Set-Content -Path $TmpHello -NoNewline -Encoding ASCII
[System.IO.File]::WriteAllBytes($TmpInit, $initElf)
[System.IO.File]::WriteAllBytes($TmpHelloElf, $helloElf)

Write-Host "[GEN] INIT ELF: $($initElf.Length) bytes (entry=0x400078)"
Write-Host "[GEN] HELLO ELF: $($helloElf.Length) bytes (entry=0x400078)"

# ── Build FAT32 image via WSL ──

function Convert-ToWslPath([string]$WindowsPath) {
    $full = [System.IO.Path]::GetFullPath($WindowsPath)
    $drive = $full.Substring(0, 1).ToLowerInvariant()
    $rest = $full.Substring(2).Replace("\", "/")
    return "/mnt/$drive$rest"
}

$OutUnix      = Convert-ToWslPath $OutputPath
$TmpHelloUnix = Convert-ToWslPath $TmpHello
$TmpInitUnix  = Convert-ToWslPath $TmpInit
$TmpHelloElfUnix = Convert-ToWslPath $TmpHelloElf

$cmd = @(
    "set -e",
    "truncate -s ${SizeMiB}M '$OutUnix'",
    "mkfs.fat -F 32 '$OutUnix'",
    "mcopy -i '$OutUnix' '$TmpHelloUnix' ::HELLO.TXT",
    "mcopy -i '$OutUnix' '$TmpInitUnix' ::INIT",
    "mmd -i '$OutUnix' ::BIN",
    "mcopy -i '$OutUnix' '$TmpHelloElfUnix' ::BIN/HELLO"
) -join "; "

wsl -d Ubuntu -- bash -c $cmd
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to create FAT32 image. Ensure WSL has dosfstools and mtools: sudo apt install dosfstools mtools"
    exit 1
}

Write-Host "[OK] Test disk created: $OutputPath"
Write-Host "      Contains: HELLO.TXT, INIT, BIN/HELLO"
