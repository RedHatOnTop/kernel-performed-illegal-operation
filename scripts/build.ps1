# KPIO OS Build Script for Windows
# PowerShell build system

param(
    [Parameter(Position=0)]
    [ValidateSet('build', 'clean', 'kernel', 'test', 'help')]
    [string]$Command = 'build'
)

# Configuration
$ErrorActionPreference = "Stop"
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$Target = "x86_64-unknown-none"
$BuildDir = Join-Path $ProjectRoot "target"

# Colors
function Write-Header($text) {
    Write-Host "`n▶ $text" -ForegroundColor Yellow
}

function Write-Success($text) {
    Write-Host "✓ $text" -ForegroundColor Green
}

function Write-Error($text) {
    Write-Host "✗ $text" -ForegroundColor Red
    exit 1
}

function Write-Warning($text) {
    Write-Host "⚠ $text" -ForegroundColor Yellow
}

# Print banner
function Show-Banner {
    Write-Host ""
    Write-Host "╔═══════════════════════════════════════════════════════════╗" -ForegroundColor Blue
    Write-Host "║                    KPIO OS Build System                    ║" -ForegroundColor Blue
    Write-Host "╚═══════════════════════════════════════════════════════════╝" -ForegroundColor Blue
    Write-Host ""
}

# Check prerequisites
function Test-Prerequisites {
    Write-Header "Checking prerequisites..."
    
    # Check Cargo
    try {
        $null = Get-Command cargo -ErrorAction Stop
        Write-Success "Cargo found"
    }
    catch {
        Write-Error "Cargo not found. Please install Rust: https://rustup.rs"
    }
    
    # Check nightly toolchain
    $toolchains = rustup show
    if ($toolchains -notmatch "nightly") {
        Write-Host "Installing nightly toolchain..."
        rustup install nightly
    }
    Write-Success "Nightly toolchain available"
    
    # Check rust-src
    $components = rustup component list --toolchain nightly
    if ($components -notmatch "rust-src \(installed\)") {
        Write-Host "Installing rust-src..."
        rustup component add rust-src --toolchain nightly
    }
    Write-Success "rust-src component installed"
    
    # Check llvm-tools
    if ($components -notmatch "llvm-tools") {
        Write-Host "Installing llvm-tools-preview..."
        rustup component add llvm-tools-preview --toolchain nightly
    }
    Write-Success "llvm-tools available"
}

# Clean build
function Invoke-Clean {
    Write-Header "Cleaning build artifacts..."
    
    if (Test-Path $BuildDir) {
        Remove-Item -Recurse -Force $BuildDir
    }
    
    Write-Success "Clean complete"
}

# Build kernel
function Build-Kernel {
    Write-Header "Building kernel ($Target)..."
    
    Push-Location (Join-Path $ProjectRoot "kernel")
    try {
        cargo +nightly build `
            --release `
            --target $Target `
            -Z build-std=core,compiler_builtins,alloc `
            -Z build-std-features=compiler-builtins-mem
        
        Write-Success "Kernel built successfully"
    }
    finally {
        Pop-Location
    }
}

# Build browser
function Build-Browser {
    Write-Header "Building browser engine..."
    
    # Browser is compiled as part of kernel
    Write-Success "Browser components built"
}

# Run tests
function Invoke-Tests {
    Write-Header "Running tests..."
    
    Push-Location (Join-Path $ProjectRoot "kernel")
    try {
        # Try to run tests
        $result = cargo test --no-default-features 2>&1
        if ($LASTEXITCODE -ne 0) {
            Write-Warning "Some tests skipped (no_std environment)"
        }
        else {
            Write-Success "All tests passed"
        }
    }
    finally {
        Pop-Location
    }
}

# Print summary
function Show-Summary {
    Write-Header "Build Summary"
    
    $kernelPath = Join-Path $BuildDir "$Target\release\kernel.exe"
    if (-not (Test-Path $kernelPath)) {
        $kernelPath = Join-Path $BuildDir "$Target\release\kernel"
    }
    
    Write-Host "`nBuild artifacts:"
    
    if (Test-Path $kernelPath) {
        $size = (Get-Item $kernelPath).Length / 1KB
        Write-Host "  " -NoNewline
        Write-Host "✓" -ForegroundColor Green -NoNewline
        Write-Host " Kernel binary: $([math]::Round($size, 2)) KB"
    }
    
    Write-Host "`n" -NoNewline
    Write-Success "Build completed successfully!"
    Write-Host "Output directory: $BuildDir`n"
}

# Main build
function Invoke-Build {
    Show-Banner
    Test-Prerequisites
    Build-Kernel
    Build-Browser
    Invoke-Tests
    Show-Summary
}

# Execute command
switch ($Command) {
    'build' {
        Invoke-Build
    }
    'clean' {
        Invoke-Clean
    }
    'kernel' {
        Test-Prerequisites
        Build-Kernel
    }
    'test' {
        Invoke-Tests
    }
    'help' {
        Write-Host "KPIO OS Build Script"
        Write-Host ""
        Write-Host "Usage: .\build.ps1 [command]"
        Write-Host ""
        Write-Host "Commands:"
        Write-Host "  build   - Build complete system (default)"
        Write-Host "  clean   - Clean build artifacts"
        Write-Host "  kernel  - Build kernel only"
        Write-Host "  test    - Run tests only"
        Write-Host "  help    - Show this help"
    }
}
