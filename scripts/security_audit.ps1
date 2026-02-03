#!/usr/bin/env pwsh
# KPIO Security Audit Runner for Windows PowerShell
# Runs comprehensive security audits for the KPIO OS

$ErrorActionPreference = "Continue"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir

# Counters
$script:TotalChecks = 0
$script:PassedChecks = 0
$script:FailedChecks = 0
$script:Warnings = 0

# Log file
$LogFile = Join-Path $ProjectRoot "security_audit_$(Get-Date -Format 'yyyyMMdd_HHmmss').log"

function Log($message) {
    Write-Host $message
    Add-Content -Path $LogFile -Value $message
}

function Pass($message) {
    $script:TotalChecks++
    $script:PassedChecks++
    Write-Host "[PASS] $message" -ForegroundColor Green
    Add-Content -Path $LogFile -Value "[PASS] $message"
}

function Fail($message) {
    $script:TotalChecks++
    $script:FailedChecks++
    Write-Host "[FAIL] $message" -ForegroundColor Red
    Add-Content -Path $LogFile -Value "[FAIL] $message"
}

function Warn($message) {
    $script:Warnings++
    Write-Host "[WARN] $message" -ForegroundColor Yellow
    Add-Content -Path $LogFile -Value "[WARN] $message"
}

function Info($message) {
    Write-Host "[INFO] $message" -ForegroundColor Cyan
    Add-Content -Path $LogFile -Value "[INFO] $message"
}

function Section($title) {
    $line = "=" * 65
    Write-Host ""
    Write-Host $line
    Write-Host " $title"
    Write-Host $line
    Add-Content -Path $LogFile -Value ""
    Add-Content -Path $LogFile -Value $line
    Add-Content -Path $LogFile -Value " $title"
    Add-Content -Path $LogFile -Value $line
}

function Test-FileContains($path, $pattern) {
    if (Test-Path $path) {
        $content = Get-ChildItem -Path $path -Recurse -Filter "*.rs" -ErrorAction SilentlyContinue | 
                   ForEach-Object { Get-Content $_.FullName -ErrorAction SilentlyContinue } | 
                   Select-String -Pattern $pattern -Quiet
        return $content
    }
    return $false
}

function Audit-KernelSecurity {
    Section "Kernel Security Audit"
    
    $securityPath = Join-Path $ProjectRoot "kernel\src\security"
    
    # KS001: Stack canaries
    if (Test-FileContains $securityPath "stack_canaries|stack-protector") {
        Pass "KS001: Stack canary implementation found"
    } else {
        Warn "KS001: Stack canary implementation not verified"
    }
    
    # KS002: ASLR
    if (Test-FileContains (Join-Path $ProjectRoot "kernel\src") "kaslr|ASLR|randomize") {
        Pass "KS002: ASLR implementation found"
    } else {
        Fail "KS002: No ASLR implementation found"
    }
    
    # KS016: Syscall filtering
    if (Test-FileContains $securityPath "SyscallFilter|syscall.*filter|seccomp") {
        Pass "KS016: Syscall filtering found"
    } else {
        Warn "KS016: No syscall filtering found"
    }
    
    # KS021: Capability model
    if (Test-FileContains $securityPath "Capability|capability") {
        Pass "KS021: Capability model found"
    } else {
        Fail "KS021: No capability model found"
    }
    
    # KS017: Audit logging
    if (Test-Path (Join-Path $securityPath "audit.rs")) {
        Pass "KS017: Audit logging module found"
    } else {
        Fail "KS017: No audit logging module"
    }
    
    # Hardening module
    if (Test-Path (Join-Path $securityPath "hardening.rs")) {
        Pass "Hardening module exists"
    } else {
        Fail "Hardening module missing"
    }
}

function Audit-BrowserSecurity {
    Section "Browser Security Audit"
    
    $browserPath = Join-Path $ProjectRoot "kpio-browser\src"
    
    # BS003: CSP
    if (Test-Path (Join-Path $browserPath "csp.rs")) {
        Pass "BS003: CSP module exists"
    } else {
        Fail "BS003: No CSP module"
    }
    
    # BS011: Cookie HttpOnly
    if (Test-FileContains $browserPath "http_only|HttpOnly") {
        Pass "BS011: HttpOnly cookie support found"
    } else {
        Warn "BS011: HttpOnly cookie support not found"
    }
    
    # BS013: SameSite cookies
    if (Test-FileContains $browserPath "same_site|SameSite") {
        Pass "BS013: SameSite cookie support found"
    } else {
        Warn "BS013: SameSite cookie support not found"
    }
    
    # BS024: Private mode
    if (Test-FileContains $browserPath "private.*mode|incognito|PrivateSession") {
        Pass "BS024: Private browsing mode found"
    } else {
        Warn "BS024: Private browsing mode not verified"
    }
}

function Audit-AppSandboxing {
    Section "App Sandboxing Audit"
    
    $securityPath = Join-Path $ProjectRoot "kernel\src\security"
    
    # AS001: Sandbox
    if (Test-Path (Join-Path $securityPath "sandbox.rs")) {
        Pass "AS001: Sandbox implementation found"
    } else {
        Fail "AS001: No sandbox implementation"
    }
    
    # Resource limits
    if (Test-Path (Join-Path $securityPath "resource.rs")) {
        Pass "Resource management module exists"
    } else {
        Warn "Resource management module missing"
    }
}

function Audit-CompilationSecurity {
    Section "Compilation Security Audit"
    
    $kernelPath = Join-Path $ProjectRoot "kernel\src"
    
    # Count unsafe blocks
    $unsafeCount = 0
    Get-ChildItem -Path $kernelPath -Recurse -Filter "*.rs" -ErrorAction SilentlyContinue | 
        ForEach-Object {
            $content = Get-Content $_.FullName -ErrorAction SilentlyContinue
            $unsafeCount += ($content | Select-String -Pattern "unsafe" -AllMatches).Matches.Count
        }
    
    Info "Found $unsafeCount lines with 'unsafe' keyword in kernel"
    
    if ($unsafeCount -lt 500) {
        Pass "Unsafe usage is within acceptable limits"
    } else {
        Warn "High number of unsafe blocks ($unsafeCount)"
    }
}

function Run-PenTest {
    Section "Penetration Testing"
    
    $pentestPath = Join-Path $ProjectRoot "tools\pentest"
    
    if (Test-Path $pentestPath) {
        Info "Penetration test framework found"
        
        Push-Location $pentestPath
        try {
            # Try to build
            $buildResult = cargo build --release 2>&1
            if ($LASTEXITCODE -eq 0) {
                Pass "Penetration test tool builds successfully"
                
                # Run tests
                $testResult = cargo test 2>&1
                if ($LASTEXITCODE -eq 0) {
                    Pass "Penetration test suite passes"
                } else {
                    Warn "Some penetration tests failed"
                }
            } else {
                Warn "Could not build penetration test tool"
            }
        } finally {
            Pop-Location
        }
    } else {
        Warn "No penetration test framework found"
    }
}

function Generate-Report {
    Section "Security Audit Summary"
    
    Write-Host ""
    Write-Host "KPIO OS Security Audit Report"
    Write-Host "Date: $(Get-Date)"
    Write-Host ""
    Write-Host ("-" * 45)
    Write-Host "Total Checks:  $script:TotalChecks"
    Write-Host "Passed:        $script:PassedChecks" -ForegroundColor Green
    Write-Host "Failed:        $script:FailedChecks" -ForegroundColor Red
    Write-Host "Warnings:      $script:Warnings" -ForegroundColor Yellow
    Write-Host ("-" * 45)
    
    if ($script:FailedChecks -eq 0) {
        Write-Host "All critical security checks passed!" -ForegroundColor Green
    } else {
        Write-Host "$script:FailedChecks critical security issue(s) found" -ForegroundColor Red
    }
    
    Write-Host ""
    Write-Host "Full log saved to: $LogFile"
}

# Main
function Main {
    Write-Host "KPIO Security Audit Runner"
    Write-Host "=========================="
    Write-Host ""
    
    Set-Location $ProjectRoot
    
    # Check Rust
    if (Get-Command rustc -ErrorAction SilentlyContinue) {
        Pass "Rust toolchain installed"
    } else {
        Fail "Rust toolchain not found"
        exit 1
    }
    
    Audit-KernelSecurity
    Audit-BrowserSecurity
    Audit-AppSandboxing
    Audit-CompilationSecurity
    Run-PenTest
    
    Generate-Report
    
    if ($script:FailedChecks -gt 0) {
        exit 1
    }
    
    exit 0
}

Main
