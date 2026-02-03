#!/bin/bash
#
# KPIO OS Release Script
# Creates a release package with all artifacts
#

set -e

# Configuration
PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:-1.0.0}"
RELEASE_DIR="${PROJECT_ROOT}/release"
RELEASE_NAME="kpio-os-${VERSION}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}"
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║                  KPIO OS Release Builder                   ║"
echo "║                     Version: ${VERSION}                        ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo -e "${NC}"

# Step 1: Clean previous release
echo -e "\n${YELLOW}▶ Preparing release directory...${NC}"
rm -rf "${RELEASE_DIR}"
mkdir -p "${RELEASE_DIR}/${RELEASE_NAME}"

# Step 2: Build the project
echo -e "\n${YELLOW}▶ Building project...${NC}"
"${PROJECT_ROOT}/scripts/build.sh" build

# Step 3: Copy build artifacts
echo -e "\n${YELLOW}▶ Copying build artifacts...${NC}"

ARTIFACT_DIR="${RELEASE_DIR}/${RELEASE_NAME}"

# Copy kernel binary
KERNEL_BIN="${PROJECT_ROOT}/target/x86_64-unknown-none/release/kernel"
if [ -f "${KERNEL_BIN}" ]; then
    cp "${KERNEL_BIN}" "${ARTIFACT_DIR}/kernel.bin"
    echo -e "${GREEN}✓ Kernel binary${NC}"
fi

# Copy ISO if exists
ISO="${PROJECT_ROOT}/target/kpio-os.iso"
if [ -f "${ISO}" ]; then
    cp "${ISO}" "${ARTIFACT_DIR}/${RELEASE_NAME}.iso"
    echo -e "${GREEN}✓ ISO image${NC}"
fi

# Step 4: Copy documentation
echo -e "\n${YELLOW}▶ Copying documentation...${NC}"

cp "${PROJECT_ROOT}/README.md" "${ARTIFACT_DIR}/"
cp "${PROJECT_ROOT}/LICENSE" "${ARTIFACT_DIR}/" 2>/dev/null || true
cp "${PROJECT_ROOT}/RELEASE_NOTES.md" "${ARTIFACT_DIR}/"
cp -r "${PROJECT_ROOT}/docs" "${ARTIFACT_DIR}/docs/" 2>/dev/null || mkdir -p "${ARTIFACT_DIR}/docs"

echo -e "${GREEN}✓ Documentation copied${NC}"

# Step 5: Generate checksums
echo -e "\n${YELLOW}▶ Generating checksums...${NC}"

cd "${ARTIFACT_DIR}"
find . -type f ! -name "*.sha256" -exec sha256sum {} \; > checksums.sha256
echo -e "${GREEN}✓ Checksums generated${NC}"

# Step 6: Create release archive
echo -e "\n${YELLOW}▶ Creating release archive...${NC}"

cd "${RELEASE_DIR}"
tar -czvf "${RELEASE_NAME}.tar.gz" "${RELEASE_NAME}"
zip -r "${RELEASE_NAME}.zip" "${RELEASE_NAME}"

# Generate archive checksums
sha256sum "${RELEASE_NAME}.tar.gz" > "${RELEASE_NAME}.tar.gz.sha256"
sha256sum "${RELEASE_NAME}.zip" > "${RELEASE_NAME}.zip.sha256"

echo -e "${GREEN}✓ Archives created${NC}"

# Step 7: Create release info
echo -e "\n${YELLOW}▶ Generating release info...${NC}"

cat > "${RELEASE_DIR}/RELEASE_INFO.txt" << EOF
KPIO OS Release ${VERSION}
========================

Release Date: $(date -u +"%Y-%m-%d %H:%M:%S UTC")

Contents:
---------
${RELEASE_NAME}.tar.gz  - Complete release (tar.gz)
${RELEASE_NAME}.zip     - Complete release (zip)

Verification:
-------------
To verify the archives:
  sha256sum -c ${RELEASE_NAME}.tar.gz.sha256
  sha256sum -c ${RELEASE_NAME}.zip.sha256

Installation:
-------------
1. Extract the archive
2. Follow instructions in QUICK_START.md
3. For detailed documentation, see docs/USER_GUIDE.md

Files Included:
---------------
$(cd "${RELEASE_NAME}" && find . -type f | sort)

EOF

echo -e "${GREEN}✓ Release info generated${NC}"

# Summary
echo -e "\n${GREEN}════════════════════════════════════════════════════════════${NC}"
echo -e "${GREEN}Release ${VERSION} created successfully!${NC}"
echo -e "${GREEN}════════════════════════════════════════════════════════════${NC}"
echo ""
echo "Release directory: ${RELEASE_DIR}"
echo ""
echo "Files created:"
ls -lh "${RELEASE_DIR}"/*.{tar.gz,zip} 2>/dev/null || echo "  (archives)"
echo ""
echo -e "${BLUE}Next steps:${NC}"
echo "1. Test the release artifacts"
echo "2. Create a Git tag: git tag -a v${VERSION} -m 'Release ${VERSION}'"
echo "3. Push the tag: git push origin v${VERSION}"
echo "4. Upload artifacts to release page"
echo ""
