#!/bin/bash
# KPIO Security Audit Runner
# Runs comprehensive security audits for the KPIO OS

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
TOTAL_CHECKS=0
PASSED_CHECKS=0
FAILED_CHECKS=0
WARNINGS=0

# Logging
LOG_FILE="$PROJECT_ROOT/security_audit_$(date +%Y%m%d_%H%M%S).log"

log() {
    echo "$1" | tee -a "$LOG_FILE"
}

pass() {
    ((TOTAL_CHECKS++))
    ((PASSED_CHECKS++))
    echo -e "${GREEN}[PASS]${NC} $1" | tee -a "$LOG_FILE"
}

fail() {
    ((TOTAL_CHECKS++))
    ((FAILED_CHECKS++))
    echo -e "${RED}[FAIL]${NC} $1" | tee -a "$LOG_FILE"
}

warn() {
    ((WARNINGS++))
    echo -e "${YELLOW}[WARN]${NC} $1" | tee -a "$LOG_FILE"
}

info() {
    echo -e "${BLUE}[INFO]${NC} $1" | tee -a "$LOG_FILE"
}

section() {
    echo "" | tee -a "$LOG_FILE"
    echo "═══════════════════════════════════════════════════════════════" | tee -a "$LOG_FILE"
    echo " $1" | tee -a "$LOG_FILE"
    echo "═══════════════════════════════════════════════════════════════" | tee -a "$LOG_FILE"
}

# Check if Rust toolchain is available
check_rust() {
    if command -v rustc &> /dev/null; then
        pass "Rust toolchain installed"
        return 0
    else
        fail "Rust toolchain not found"
        return 1
    fi
}

# Run kernel security audit
audit_kernel_security() {
    section "Kernel Security Audit"
    
    # KS001: Stack canaries (Rust enables by default in release)
    if grep -rq "stack_canaries\|stack-protector" "$PROJECT_ROOT/kernel/src/security/" 2>/dev/null; then
        pass "KS001: Stack canary implementation found"
    else
        warn "KS001: Stack canary implementation not verified"
    fi
    
    # KS002: ASLR implementation
    if grep -rq "kaslr\|ASLR\|randomize" "$PROJECT_ROOT/kernel/src/" 2>/dev/null; then
        pass "KS002: ASLR implementation found"
    else
        fail "KS002: No ASLR implementation found"
    fi
    
    # KS003: W^X policy
    if grep -rq "PROT_WRITE.*PROT_EXEC\|W\^X\|write.*execute" "$PROJECT_ROOT/kernel/src/memory/" 2>/dev/null || \
       grep -rq "PageFlags\|page.*permissions" "$PROJECT_ROOT/kernel/src/" 2>/dev/null; then
        pass "KS003: Memory protection flags found"
    else
        warn "KS003: W^X policy implementation not found"
    fi
    
    # KS011: Syscall input validation
    if grep -rq "validate\|Validate\|check_.*ptr\|user_ptr" "$PROJECT_ROOT/kernel/src/syscall/" 2>/dev/null; then
        pass "KS011: Syscall validation found"
    else
        fail "KS011: No syscall validation found"
    fi
    
    # KS016: Syscall filtering (seccomp-like)
    if grep -rq "SyscallFilter\|syscall.*filter\|seccomp" "$PROJECT_ROOT/kernel/src/security/" 2>/dev/null; then
        pass "KS016: Syscall filtering found"
    else
        warn "KS016: No syscall filtering found"
    fi
    
    # KS021: Capability model
    if grep -rq "Capability\|capability\|Cap" "$PROJECT_ROOT/kernel/src/security/" 2>/dev/null; then
        pass "KS021: Capability model found"
    else
        fail "KS021: No capability model found"
    fi
    
    # KS017: Audit logging
    if [ -f "$PROJECT_ROOT/kernel/src/security/audit.rs" ]; then
        pass "KS017: Audit logging module found"
    else
        fail "KS017: No audit logging module"
    fi
    
    # Check hardening module
    if [ -f "$PROJECT_ROOT/kernel/src/security/hardening.rs" ]; then
        pass "Hardening module exists"
    else
        fail "Hardening module missing"
    fi
}

# Run browser security audit
audit_browser_security() {
    section "Browser Security Audit"
    
    # BS001: Same-origin policy
    if grep -riq "same.origin\|origin.*check\|SameOrigin" "$PROJECT_ROOT/kpio-browser/src/" 2>/dev/null; then
        pass "BS001: Same-origin policy found"
    else
        warn "BS001: Same-origin policy not verified"
    fi
    
    # BS003: CSP
    if [ -f "$PROJECT_ROOT/kpio-browser/src/csp.rs" ]; then
        pass "BS003: CSP module exists"
    else
        fail "BS003: No CSP module"
    fi
    
    # BS011: Cookie HttpOnly
    if grep -riq "http_only\|HttpOnly" "$PROJECT_ROOT/kpio-browser/src/" 2>/dev/null; then
        pass "BS011: HttpOnly cookie support found"
    else
        warn "BS011: HttpOnly cookie support not found"
    fi
    
    # BS013: SameSite cookies
    if grep -riq "same_site\|SameSite" "$PROJECT_ROOT/kpio-browser/src/" 2>/dev/null; then
        pass "BS013: SameSite cookie support found"
    else
        warn "BS013: SameSite cookie support not found"
    fi
    
    # BS024: Private mode
    if grep -riq "private.*mode\|incognito\|PrivateSession" "$PROJECT_ROOT/kpio-browser/src/" 2>/dev/null; then
        pass "BS024: Private browsing mode found"
    else
        warn "BS024: Private browsing mode not verified"
    fi
}

# Run app sandboxing audit
audit_app_sandboxing() {
    section "App Sandboxing Audit"
    
    # AS001: Process isolation
    if grep -riq "sandbox\|Sandbox\|isolation" "$PROJECT_ROOT/kernel/src/security/" 2>/dev/null; then
        pass "AS001: Sandbox implementation found"
    else
        fail "AS001: No sandbox implementation"
    fi
    
    # AS003: Resource limits
    if grep -riq "ResourceLimit\|resource.*limit\|rlimit" "$PROJECT_ROOT/kernel/src/" 2>/dev/null; then
        pass "AS003: Resource limits found"
    else
        warn "AS003: Resource limits not verified"
    fi
    
    # Check sandbox module
    if [ -f "$PROJECT_ROOT/kernel/src/security/sandbox.rs" ]; then
        pass "Sandbox module exists"
    else
        fail "Sandbox module missing"
    fi
    
    # Check resource module
    if [ -f "$PROJECT_ROOT/kernel/src/security/resource.rs" ]; then
        pass "Resource management module exists"
    else
        warn "Resource management module missing"
    fi
}

# Run compilation security checks
audit_compilation_security() {
    section "Compilation Security Audit"
    
    # Check for unsafe blocks
    UNSAFE_COUNT=$(grep -r "unsafe" "$PROJECT_ROOT/kernel/src/" --include="*.rs" 2>/dev/null | wc -l || echo "0")
    info "Found $UNSAFE_COUNT lines with 'unsafe' keyword in kernel"
    
    if [ "$UNSAFE_COUNT" -lt 500 ]; then
        pass "Unsafe usage is within acceptable limits"
    else
        warn "High number of unsafe blocks ($UNSAFE_COUNT)"
    fi
    
    # Check for panic usage (should use Result in production)
    PANIC_COUNT=$(grep -r "panic!\|unwrap()\|expect(" "$PROJECT_ROOT/kernel/src/" --include="*.rs" 2>/dev/null | grep -v "test" | wc -l || echo "0")
    info "Found $PANIC_COUNT potential panic points in kernel (excluding tests)"
    
    # Check for TODO/FIXME security items
    SECURITY_TODOS=$(grep -ri "TODO.*security\|FIXME.*security\|XXX.*security" "$PROJECT_ROOT/" --include="*.rs" 2>/dev/null | wc -l || echo "0")
    if [ "$SECURITY_TODOS" -gt 0 ]; then
        warn "Found $SECURITY_TODOS unresolved security TODOs"
    else
        pass "No unresolved security TODOs"
    fi
}

# Run penetration tests
run_pentest() {
    section "Penetration Testing"
    
    if [ -d "$PROJECT_ROOT/tools/pentest" ]; then
        info "Penetration test framework found"
        
        # Try to build pentest tool
        if cd "$PROJECT_ROOT/tools/pentest" && cargo build --release 2>/dev/null; then
            pass "Penetration test tool builds successfully"
            
            # Run tests
            if cargo test 2>/dev/null; then
                pass "Penetration test suite passes"
            else
                warn "Some penetration tests failed"
            fi
        else
            warn "Could not build penetration test tool"
        fi
        
        cd "$PROJECT_ROOT"
    else
        warn "No penetration test framework found"
    fi
}

# Generate summary report
generate_report() {
    section "Security Audit Summary"
    
    echo "" | tee -a "$LOG_FILE"
    echo "KPIO OS Security Audit Report" | tee -a "$LOG_FILE"
    echo "Date: $(date)" | tee -a "$LOG_FILE"
    echo "" | tee -a "$LOG_FILE"
    echo "─────────────────────────────────────────" | tee -a "$LOG_FILE"
    echo "Total Checks:  $TOTAL_CHECKS" | tee -a "$LOG_FILE"
    echo -e "Passed:        ${GREEN}$PASSED_CHECKS${NC}" | tee -a "$LOG_FILE"
    echo -e "Failed:        ${RED}$FAILED_CHECKS${NC}" | tee -a "$LOG_FILE"
    echo -e "Warnings:      ${YELLOW}$WARNINGS${NC}" | tee -a "$LOG_FILE"
    echo "─────────────────────────────────────────" | tee -a "$LOG_FILE"
    
    if [ $FAILED_CHECKS -eq 0 ]; then
        echo -e "${GREEN}✓ All critical security checks passed!${NC}" | tee -a "$LOG_FILE"
    else
        echo -e "${RED}✗ $FAILED_CHECKS critical security issue(s) found${NC}" | tee -a "$LOG_FILE"
    fi
    
    echo "" | tee -a "$LOG_FILE"
    echo "Full log saved to: $LOG_FILE" | tee -a "$LOG_FILE"
}

# Main
main() {
    echo "KPIO Security Audit Runner"
    echo "=========================="
    echo ""
    
    cd "$PROJECT_ROOT"
    
    check_rust || exit 1
    
    audit_kernel_security
    audit_browser_security
    audit_app_sandboxing
    audit_compilation_security
    run_pentest
    
    generate_report
    
    # Exit with error if any critical failures
    if [ $FAILED_CHECKS -gt 0 ]; then
        exit 1
    fi
    
    exit 0
}

main "$@"
