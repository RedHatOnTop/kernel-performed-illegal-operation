<#
.SYNOPSIS
    KPIO Kernel QEMU Boot Test Runner

.DESCRIPTION
    Build kernel -> Create disk image -> Run QEMU -> Capture serial output -> Verify results
    Fully automated terminal-only testing infrastructure.
    
    Boot method: UEFI pflash via edk2 firmware (QEMU 10.x compatible)
    UEFI pflash is the recommended boot method — BIOS boot has a known FAT parser
    overflow in bootloader 0.11.14 (see docs/known-issues.md).
    Capture: -display none -serial file: (headless CI mode)
    
    Test modes:
      boot    - Verify kernel boot + core init (default)
      smoke   - Boot + all subsystem initialization verification
      linux   - Phase 7-4 Linux compatibility layer verification
      io      - End-to-end I/O integration (VirtIO NIC + block + VFS)
      full    - All verification combined
      custom  - Check strings specified with -Expect

.PARAMETER Mode
    Test mode (boot, smoke, linux, full, custom)

.PARAMETER Timeout
    QEMU timeout in seconds (default: 45)

.PARAMETER Release
    Use release build

.PARAMETER NoBuild
    Skip kernel build (use existing binary)

.PARAMETER NoImage
    Skip image build (use existing disk image)

.PARAMETER Expect
    Strings to check in custom mode

.PARAMETER Verbose
    Show detailed output including serial log

.PARAMETER KeepLog
    Keep serial log after test

.EXAMPLE
    .\scripts\qemu-test.ps1                          # Default boot test
    .\scripts\qemu-test.ps1 -Mode smoke              # Smoke test
    .\scripts\qemu-test.ps1 -Mode linux              # Linux compat verification
    .\scripts\qemu-test.ps1 -Mode io                 # Full I/O integration test
    .\scripts\qemu-test.ps1 -Mode io -Verbose        # I/O test with serial log
    .\scripts\qemu-test.ps1 -Mode full -Verbose      # Full verification (verbose)
    .\scripts\qemu-test.ps1 -Mode custom -Expect "GDT initialized","IDT initialized"
    .\scripts\qemu-test.ps1 -NoBuild -NoImage -Mode boot  # Quick retest
#>

param(
    [ValidateSet("boot", "smoke", "linux", "full", "io", "custom")]
    [string]$Mode = "boot",
    [int]$Timeout = 45,
    [switch]$Release,
    [switch]$NoBuild,
    [switch]$NoImage,
    [string[]]$Expect = @(),
    [switch]$Verbose,
    [switch]$KeepLog,
    [string]$TestDisk = ""
)

$ErrorActionPreference = "Stop"

# ============================================================
# Configuration
# ============================================================

$ProjectRoot = Split-Path -Parent $PSScriptRoot
$KernelTarget = "x86_64-unknown-none"
$Profile = if ($Release) { "release" } else { "debug" }
$TargetDir = Join-Path $ProjectRoot "target\$KernelTarget\$Profile"
$KernelPath = Join-Path $TargetDir "kernel"
$SerialLog = Join-Path $ProjectRoot "target\qemu-test-serial.log"
$ResultFile = Join-Path $ProjectRoot "target\qemu-test-result.json"

$TestStart = Get-Date
$TestId = $TestStart.ToString("yyyyMMdd-HHmmss")

# ============================================================
# Utility Functions
# ============================================================

function Write-Step($step, $total, $msg) {
    Write-Host "[$step/$total] $msg" -ForegroundColor Yellow
}

function Write-Pass($msg) {
    Write-Host "  [PASS] $msg" -ForegroundColor Green
}

function Write-Fail($msg) {
    Write-Host "  [FAIL] $msg" -ForegroundColor Red
}

function Write-Info($msg) {
    Write-Host "  [INFO] $msg" -ForegroundColor Gray
}

function Write-Detail($msg) {
    if ($Verbose) { Write-Host "         $msg" -ForegroundColor DarkGray }
}

# ============================================================
# Test Check Definitions
# ============================================================

# Boot checks: kernel entry + fundamental initialization
$BootChecks = @(
    @{ Pattern = "Hello, Kernel";              Label = "Kernel entry point" },
    @{ Pattern = "GDT initialized";            Label = "GDT init" },
    @{ Pattern = "IDT initialized";            Label = "IDT init" },
    @{ Pattern = "Physical memory offset";     Label = "Physical memory mapping" },
    @{ Pattern = "Heap initialized";           Label = "Heap init" }
)

# Smoke checks: all subsystems up to APIC (ACPI has known page fault)
$SmokeChecks = $BootChecks + @(
    @{ Pattern = "Scheduler initialized";      Label = "Scheduler init" },
    @{ Pattern = "Process table initialized";  Label = "Process table" },
    @{ Pattern = "Terminal subsystem ready";   Label = "Terminal subsystem" },
    @{ Pattern = "VFS initialized";            Label = "VFS init" },
    @{ Pattern = "Network stack ready";        Label = "Network stack" },
    @{ Pattern = "APIC initialized";           Label = "APIC init" }
)

# Linux compat checks: verify no regressions from Phase 7-4
$LinuxChecks = $SmokeChecks + @(
    @{ Pattern = "Unrecoverable page fault";   Label = "Known ACPI fault (expected)"; ExpectFound = $true },
    @{ Pattern = "System halted";              Label = "Proper halt after fault" }
)

# I/O integration checks: end-to-end VirtIO NIC + block verification (Phase 9-5)
$IoChecks = $SmokeChecks + @(
    @{ Pattern = "NIC initialized successfully";  Label = "VirtIO NIC init" },
    @{ Pattern = "Lease acquired";                Label = "DHCP success" },
    @{ Pattern = "VirtIO Net.*TX:";               Label = "Packet TX"; IsRegex = $true },
    @{ Pattern = "VFS.*Mounted";                  Label = "VFS mount"; IsRegex = $true },
    @{ Pattern = "Self-test.*read.*bytes";         Label = "VFS read"; IsRegex = $true },
    @{ Pattern = "E2E.*PASSED";                   Label = "E2E integration test"; IsRegex = $true }
)

# ============================================================
# Step 1: Prerequisites
# ============================================================

$TotalSteps = 5
if ($NoBuild) { $TotalSteps-- }
if ($NoImage) { $TotalSteps-- }
$CurrentStep = 0

$CurrentStep++
Write-Step $CurrentStep $TotalSteps "Checking prerequisites"

# Find QEMU
$QemuExe = $null
$qemuCmd = Get-Command "qemu-system-x86_64" -ErrorAction SilentlyContinue
if ($qemuCmd) {
    $QemuExe = $qemuCmd.Source
} else {
    $defaultPaths = @(
        "C:\Program Files\qemu\qemu-system-x86_64.exe",
        "C:\Program Files\QEMU\qemu-system-x86_64.exe"
    )
    foreach ($p in $defaultPaths) {
        if (Test-Path $p) { $QemuExe = $p; break }
    }
}
if (-not $QemuExe) {
    Write-Fail "QEMU not installed"
    Write-Host "  Install: winget install QEMU.QEMU" -ForegroundColor Cyan
    exit 1
}
Write-Pass "QEMU: $QemuExe"

# Find OVMF (UEFI firmware)
$OvmfPath = $null
$OvmfSearchPaths = @(
    "C:\Program Files\qemu\share\edk2-x86_64-code.fd",
    "C:\Program Files\QEMU\share\edk2-x86_64-code.fd",
    "C:\Program Files\qemu\share\OVMF.fd",
    "C:\Program Files\QEMU\share\OVMF.fd",
    "$env:USERPROFILE\scoop\apps\qemu\current\share\edk2-x86_64-code.fd",
    "$env:USERPROFILE\.kpio\OVMF.fd"
)
foreach ($p in $OvmfSearchPaths) {
    if (Test-Path $p) { $OvmfPath = $p; break }
}
if (-not $OvmfPath) {
    Write-Fail "OVMF (UEFI firmware) not found"
    exit 1
}
Write-Pass "OVMF: $OvmfPath"

# ============================================================
# Step 2: Kernel Build
# ============================================================

if (-not $NoBuild) {
    $CurrentStep++
    Write-Step $CurrentStep $TotalSteps "Building kernel ($Profile)"

    Push-Location $ProjectRoot
    try {
        $buildArgs = @("build", "-p", "kpio-kernel")
        if ($Release) { $buildArgs += "--release" }

        $buildProc = Start-Process -FilePath "cargo" -ArgumentList $buildArgs `
            -NoNewWindow -Wait -PassThru `
            -RedirectStandardError (Join-Path $ProjectRoot "target\build-stderr.log")

        if ($buildProc.ExitCode -ne 0) {
            Write-Fail "Kernel build failed (exit code: $($buildProc.ExitCode))"
            Write-Host "  Log: target\build-stderr.log" -ForegroundColor Gray
            exit 1
        }
    } finally {
        Pop-Location
    }

    if (-not (Test-Path $KernelPath)) {
        Write-Fail "Kernel binary not found: $KernelPath"
        exit 1
    }
    $kernelSize = (Get-Item $KernelPath).Length / 1MB
    Write-Pass ("Kernel built ({0:F1} MB)" -f $kernelSize)
}

# ============================================================
# Step 3: Boot Image (UEFI disk image via bootloader crate)
# ============================================================

$BootMethod = "uefi-pflash"

if (-not $NoImage) {
    $CurrentStep++
    Write-Step $CurrentStep $TotalSteps "Preparing boot image"

    $ImageBuilder = $null
    $builderPaths = @(
        (Join-Path $ProjectRoot "tools\boot\target\x86_64-pc-windows-msvc\release\build-image.exe"),
        (Join-Path $ProjectRoot "tools\boot\target\release\build-image.exe")
    )
    foreach ($bp in $builderPaths) {
        if (Test-Path $bp) { $ImageBuilder = $bp; break }
    }

    if ($ImageBuilder) {
        Write-Detail "Image builder: $ImageBuilder"
        $imgProc = Start-Process -FilePath $ImageBuilder -ArgumentList $KernelPath `
            -NoNewWindow -Wait -PassThru
        if ($imgProc.ExitCode -ne 0) {
            Write-Fail "Image build failed"
            exit 1
        }
        $UefiImage = Join-Path $TargetDir "kpio-uefi.img"
        if (Test-Path $UefiImage) {
            $imgSize = (Get-Item $UefiImage).Length / 1MB
            Write-Pass ("UEFI disk image ready ({0:F1} MB)" -f $imgSize)
        } else {
            Write-Fail "UEFI image not found after build"
            exit 1
        }
    } else {
        Write-Fail "Image builder not found. Build it first:"
        Write-Host "  cd tools\boot && cargo build --release" -ForegroundColor Cyan
        exit 1
    }
} else {
    $UefiImage = Join-Path $TargetDir "kpio-uefi.img"
    if (-not (Test-Path $UefiImage)) {
        Write-Fail "UEFI image not found: $UefiImage"
        Write-Host "  Run without -NoImage to build it" -ForegroundColor Cyan
        exit 1
    }
}

# ============================================================
# Step 4: Run QEMU
# ============================================================

$CurrentStep++
Write-Step $CurrentStep $TotalSteps "Running QEMU (timeout: ${Timeout}s, mode: $Mode)"

# Kill any existing QEMU processes
Get-Process -Name "qemu-system-x86_64" -ErrorAction SilentlyContinue | `
    Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

# Remove previous log
if (Test-Path $SerialLog) { Remove-Item $SerialLog -Force }

# Build QEMU argument string
# IMPORTANT: QEMU 10.x requires pflash for UEFI firmware (not -bios).
# UEFI pflash is the recommended boot method for KPIO OS.
# BIOS boot has a known FAT parser overflow in bootloader 0.11.14 — see docs/known-issues.md.
# Use -display none + -serial file: for headless CI-compatible capture
$argParts = @(
    "-machine q35",
    "-cpu `"qemu64,+rdrand`"",
    "-m 512M",
    "-display none",
    "-monitor none",
    "-serial `"file:$SerialLog`"",
    "-device `"isa-debug-exit,iobase=0xf4,iosize=0x04`"",
    "-netdev `"user,id=net0`"",
    "-device `"virtio-net-pci,netdev=net0`"",
    "-no-reboot",
    "-drive `"if=pflash,format=raw,readonly=on,file=$OvmfPath`"",
    "-drive `"format=raw,file=$UefiImage`""
)

# Auto-attach test disk in io mode if not explicitly specified
if ($Mode -eq "io" -and -not $TestDisk) {
    $DefaultTestDisk = Join-Path $ProjectRoot "tests\e2e\test-disk.img"
    if (Test-Path $DefaultTestDisk) {
        $TestDisk = $DefaultTestDisk
        Write-Detail "Auto-attaching test disk for io mode: $TestDisk"
    } else {
        Write-Info "Test disk not found at $DefaultTestDisk — run .\scripts\create-test-disk.ps1 first"
    }
}

if ($TestDisk -and (Test-Path $TestDisk)) {
    $argParts += "-drive `"file=$TestDisk,format=raw,if=none,id=testdisk`""
    $argParts += "-device `"virtio-blk-pci,drive=testdisk`""
    Write-Detail "Attached test disk: $TestDisk"
}
$QemuArgString = $argParts -join " "

Write-Detail "QEMU: $QemuExe"
Write-Detail "Args: $QemuArgString"

# Run QEMU as background process
$qemuProc = Start-Process -FilePath $QemuExe -ArgumentList $QemuArgString `
    -NoNewWindow -PassThru

$elapsed = 0
$pollInterval = 1
$earlyExit = $false

Write-Host "  QEMU PID: $($qemuProc.Id)" -ForegroundColor DarkGray

while ($elapsed -lt $Timeout) {
    Start-Sleep -Seconds $pollInterval
    $elapsed += $pollInterval

    if ($qemuProc.HasExited) {
        $earlyExit = $true
        Write-Detail "QEMU exited (${elapsed}s, exit code: $($qemuProc.ExitCode))"
        break
    }

    if ($Verbose -and ($elapsed % 5 -eq 0)) {
        $logSize = if (Test-Path $SerialLog) { (Get-Item $SerialLog).Length } else { 0 }
        Write-Detail "  ${elapsed}s elapsed... (log: $logSize bytes)"
    }

    # Check serial log for early termination signals
    if (Test-Path $SerialLog) {
        $content = Get-Content $SerialLog -Raw -ErrorAction SilentlyContinue
        if ($content) {
            if ($content.Contains("Kernel initialization complete")) {
                Write-Detail "Kernel init complete detected -> waiting 2s"
                Start-Sleep -Seconds 2
                $earlyExit = $true
                break
            }
            if ($content -match "System halted|panicked at") {
                Write-Detail "System halted / panic detected -> stopping"
                Start-Sleep -Seconds 1
                $earlyExit = $true
                break
            }
        }
    }
}

# Kill QEMU if still running
if (-not $qemuProc.HasExited) {
    Write-Detail "Timeout -> stopping QEMU"
    Stop-Process -Id $qemuProc.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 1
}

$QemuExitCode = if ($qemuProc.HasExited) { $qemuProc.ExitCode } else { -1 }

# ============================================================
# Step 5: Verify Results
# ============================================================

$CurrentStep++
Write-Step $CurrentStep $TotalSteps "Verifying results (mode: $Mode)"

# Read serial log
$SerialContent = ""
if (Test-Path $SerialLog) {
    $SerialContent = Get-Content $SerialLog -Raw -ErrorAction SilentlyContinue
    if (-not $SerialContent) { $SerialContent = "" }
}

$logLines = if ($SerialContent.Length -gt 0) { ($SerialContent -split "`n").Count } else { 0 }
Write-Info "Serial log: $logLines lines, $($SerialContent.Length) bytes"

if ($Verbose -and $SerialContent.Length -gt 0) {
    Write-Host ""
    Write-Host "  ---- Serial Output ----" -ForegroundColor DarkCyan
    $SerialContent -split "`n" | ForEach-Object {
        Write-Host "  | $_" -ForegroundColor DarkGray
    }
    Write-Host "  -----------------------" -ForegroundColor DarkCyan
    Write-Host ""
}

# Select checks based on mode
$checks = switch ($Mode) {
    "boot"   { $BootChecks }
    "smoke"  { $SmokeChecks }
    "linux"  { $LinuxChecks }
    "full"   { $LinuxChecks }
    "io"     { $IoChecks }
    "custom" {
        $Expect | ForEach-Object {
            @{ Pattern = $_; Label = "Custom: $_" }
        }
    }
}

$totalChecks = $checks.Count
$passCount = 0
$failCount = 0
$results = @()

foreach ($check in $checks) {
    $pattern = $check.Pattern
    $label = $check.Label
    $isRegex = if ($check.ContainsKey("IsRegex")) { $check.IsRegex } else { $false }

    if ($isRegex) {
        $found = $SerialContent -match $pattern
    } else {
        $found = $SerialContent.Contains($pattern)
    }
    $pass = $found

    if ($pass) {
        Write-Pass $label
        $passCount++
        $results += @{ Check = $label; Status = "PASS" }
    } else {
        Write-Fail $label
        $failCount++
        $results += @{ Check = $label; Status = "FAIL" }
    }
}

# ============================================================
# Summary
# ============================================================

Write-Host ""
Write-Host "================================================" -ForegroundColor Cyan
Write-Host " QEMU Test Results ($Mode)" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan
Write-Host ""

$duration = (Get-Date) - $TestStart
Write-Host "  Test ID:        $TestId"
Write-Host "  Mode:           $Mode"
Write-Host "  Profile:        $Profile"
Write-Host "  Boot method:    $BootMethod"
Write-Host "  QEMU exit code: $QemuExitCode"
Write-Host "  Duration:       $([math]::Round($duration.TotalSeconds, 1))s"
Write-Host "  Serial log:     $logLines lines"
Write-Host ""

$overallPass = ($failCount -eq 0) -and ($SerialContent.Length -gt 0)

if ($overallPass) {
    Write-Host "  Result: ALL PASS ($passCount/$totalChecks)" -ForegroundColor Green
} elseif ($SerialContent.Length -eq 0) {
    Write-Host "  Result: NO OUTPUT (QEMU produced no serial output)" -ForegroundColor Red
    $overallPass = $false
} else {
    Write-Host "  Result: FAILED ($failCount/$totalChecks failed)" -ForegroundColor Red
}

Write-Host ""
Write-Host "================================================" -ForegroundColor Cyan

# Save JSON result
$resultObj = @{
    testId       = $TestId
    mode         = $Mode
    profile      = $Profile
    bootMethod   = $BootMethod
    qemuExitCode = $QemuExitCode
    duration     = [math]::Round($duration.TotalSeconds, 1)
    serialLines  = $logLines
    serialBytes  = $SerialContent.Length
    pass         = $overallPass
    totalChecks  = $totalChecks
    passCount    = $passCount
    failCount    = $failCount
    checks       = $results
}
$resultObj | ConvertTo-Json -Depth 3 | Out-File $ResultFile -Encoding UTF8

Write-Info "Result file: $ResultFile"
if (Test-Path $SerialLog) {
    Write-Info "Serial log: $SerialLog"
}

if ($overallPass) {
    exit 0
} else {
    exit 1
}
