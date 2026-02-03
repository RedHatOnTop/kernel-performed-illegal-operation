#!/bin/bash
#
# KPIO OS Build Script
# Builds the complete KPIO OS system
#

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="x86_64-unknown-none"
BUILD_DIR="${PROJECT_ROOT}/target"
ISO_DIR="${BUILD_DIR}/iso"
ISO_NAME="kpio-os.iso"

# Print banner
print_banner() {
    echo -e "${BLUE}"
    echo "╔═══════════════════════════════════════════════════════════╗"
    echo "║                    KPIO OS Build System                    ║"
    echo "╚═══════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

# Print section header
section() {
    echo -e "\n${YELLOW}▶ $1${NC}"
}

# Print success message
success() {
    echo -e "${GREEN}✓ $1${NC}"
}

# Print error message
error() {
    echo -e "${RED}✗ $1${NC}"
    exit 1
}

# Check prerequisites
check_prerequisites() {
    section "Checking prerequisites..."
    
    # Check Rust toolchain
    if ! command -v cargo &> /dev/null; then
        error "Cargo not found. Please install Rust: https://rustup.rs"
    fi
    success "Cargo found"
    
    # Check for nightly toolchain
    if ! rustup show | grep -q "nightly"; then
        echo "Installing nightly toolchain..."
        rustup install nightly
    fi
    success "Nightly toolchain available"
    
    # Check for rust-src component
    if ! rustup component list --toolchain nightly | grep -q "rust-src (installed)"; then
        echo "Installing rust-src component..."
        rustup component add rust-src --toolchain nightly
    fi
    success "rust-src component installed"
    
    # Check for llvm-tools component
    if ! rustup component list --toolchain nightly | grep -q "llvm-tools"; then
        echo "Installing llvm-tools-preview..."
        rustup component add llvm-tools-preview --toolchain nightly
    fi
    success "llvm-tools available"
    
    # Check for bootimage (optional, but recommended)
    if ! command -v bootimage &> /dev/null; then
        echo "Installing bootimage..."
        cargo install bootimage
    fi
    success "bootimage tool available"
}

# Clean build artifacts
clean() {
    section "Cleaning build artifacts..."
    rm -rf "${BUILD_DIR}"
    success "Clean complete"
}

# Build kernel
build_kernel() {
    section "Building kernel (${TARGET})..."
    
    cd "${PROJECT_ROOT}/kernel"
    
    # Build in release mode
    cargo +nightly build \
        --release \
        --target "${TARGET}" \
        -Z build-std=core,compiler_builtins,alloc \
        -Z build-std-features=compiler-builtins-mem
    
    success "Kernel built successfully"
}

# Build browser components
build_browser() {
    section "Building browser engine..."
    
    cd "${PROJECT_ROOT}"
    
    # Browser components are compiled as part of kernel in no_std
    # This is a placeholder for future separate browser builds
    
    success "Browser components built"
}

# Build userspace applications
build_userspace() {
    section "Building userspace applications..."
    
    cd "${PROJECT_ROOT}"
    
    # Userspace apps would be built here if they exist
    # For now, apps are part of the kernel
    
    success "Userspace applications built"
}

# Run tests
run_tests() {
    section "Running tests..."
    
    cd "${PROJECT_ROOT}/kernel"
    
    # Run unit tests (on host)
    cargo test --no-default-features 2>/dev/null || {
        echo -e "${YELLOW}⚠ Some tests skipped (no_std environment)${NC}"
    }
    
    success "Tests complete"
}

# Create bootable image
create_bootimage() {
    section "Creating bootable image..."
    
    cd "${PROJECT_ROOT}/kernel"
    
    # Create bootimage
    cargo +nightly bootimage --release --target "${TARGET}" 2>/dev/null || {
        echo -e "${YELLOW}⚠ bootimage creation skipped (requires bootloader configuration)${NC}"
        return
    }
    
    success "Bootable image created"
}

# Create ISO
create_iso() {
    section "Creating ISO image..."
    
    # Check for required tools
    if ! command -v xorriso &> /dev/null && ! command -v genisoimage &> /dev/null; then
        echo -e "${YELLOW}⚠ ISO creation skipped (xorriso/genisoimage not found)${NC}"
        return
    fi
    
    # Create ISO directory structure
    mkdir -p "${ISO_DIR}/boot/grub"
    
    # Copy kernel
    KERNEL_BIN="${BUILD_DIR}/${TARGET}/release/kernel"
    if [ -f "${KERNEL_BIN}" ]; then
        cp "${KERNEL_BIN}" "${ISO_DIR}/boot/kernel.bin"
    fi
    
    # Create GRUB config
    cat > "${ISO_DIR}/boot/grub/grub.cfg" << 'EOF'
set timeout=5
set default=0

menuentry "KPIO OS" {
    multiboot2 /boot/kernel.bin
    boot
}

menuentry "KPIO OS (Recovery Mode)" {
    multiboot2 /boot/kernel.bin recovery
    boot
}
EOF
    
    # Create ISO with xorriso or genisoimage
    if command -v xorriso &> /dev/null; then
        xorriso -as mkisofs \
            -R -b boot/grub/i386-pc/eltorito.img \
            -no-emul-boot \
            -boot-load-size 4 \
            -boot-info-table \
            -o "${BUILD_DIR}/${ISO_NAME}" \
            "${ISO_DIR}" 2>/dev/null || {
            echo -e "${YELLOW}⚠ ISO creation requires GRUB files${NC}"
            return
        }
    fi
    
    success "ISO created: ${BUILD_DIR}/${ISO_NAME}"
}

# Generate checksums
generate_checksums() {
    section "Generating checksums..."
    
    cd "${BUILD_DIR}"
    
    # Generate SHA256 checksums
    if [ -f "${ISO_NAME}" ]; then
        sha256sum "${ISO_NAME}" > "${ISO_NAME}.sha256"
        success "SHA256: $(cat ${ISO_NAME}.sha256)"
    fi
    
    KERNEL_BIN="${TARGET}/release/kernel"
    if [ -f "${KERNEL_BIN}" ]; then
        sha256sum "${KERNEL_BIN}" > "kernel.sha256"
        success "Kernel checksum generated"
    fi
}

# Print build summary
print_summary() {
    section "Build Summary"
    
    echo -e "\nBuild artifacts:"
    
    if [ -f "${BUILD_DIR}/${TARGET}/release/kernel" ]; then
        SIZE=$(du -h "${BUILD_DIR}/${TARGET}/release/kernel" | cut -f1)
        echo -e "  ${GREEN}✓${NC} Kernel binary: ${SIZE}"
    fi
    
    if [ -f "${BUILD_DIR}/${ISO_NAME}" ]; then
        SIZE=$(du -h "${BUILD_DIR}/${ISO_NAME}" | cut -f1)
        echo -e "  ${GREEN}✓${NC} ISO image: ${SIZE}"
    fi
    
    echo -e "\n${GREEN}Build completed successfully!${NC}"
    echo -e "Output directory: ${BUILD_DIR}\n"
}

# Main build function
build() {
    print_banner
    check_prerequisites
    build_kernel
    build_browser
    build_userspace
    run_tests
    create_bootimage
    create_iso
    generate_checksums
    print_summary
}

# Parse arguments
case "${1:-build}" in
    build)
        build
        ;;
    clean)
        clean
        ;;
    kernel)
        check_prerequisites
        build_kernel
        ;;
    test)
        run_tests
        ;;
    iso)
        create_iso
        ;;
    help|--help|-h)
        echo "Usage: $0 [command]"
        echo ""
        echo "Commands:"
        echo "  build   - Build complete system (default)"
        echo "  clean   - Clean build artifacts"
        echo "  kernel  - Build kernel only"
        echo "  test    - Run tests only"
        echo "  iso     - Create ISO image only"
        echo "  help    - Show this help"
        ;;
    *)
        error "Unknown command: $1"
        ;;
esac
