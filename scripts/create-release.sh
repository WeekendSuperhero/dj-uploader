#!/bin/bash

# Script to build and create a GitHub release
# Requires: gh CLI tool (brew install gh)

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

log_info() {
    echo -e "${BLUE}ℹ ${1}${NC}"
}

log_success() {
    echo -e "${GREEN}✓ ${1}${NC}"
}

log_error() {
    echo -e "${RED}✗ ${1}${NC}"
}

# Read version from Cargo.toml
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
cd "$PROJECT_ROOT"

if [ -f "Cargo.toml" ]; then
    VERSION=$(grep -E '^version = ' "Cargo.toml" | head -1 | sed -E 's/version = "(.*)"/\1/')
else
    log_error "Could not find Cargo.toml"
    exit 1
fi

TAG="v${VERSION}"
DMG_FILE="target/DJ-Uploader-${VERSION}.dmg"

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
log_info "Creating GitHub Release for DJ Uploader ${VERSION}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Check if gh CLI is installed
if ! command -v gh &> /dev/null; then
    log_error "GitHub CLI (gh) is not installed"
    echo ""
    echo "Install with: brew install gh"
    echo "Then run: gh auth login"
    exit 1
fi

# Check if authenticated
if ! gh auth status &> /dev/null; then
    log_error "Not authenticated with GitHub"
    echo ""
    echo "Run: gh auth login"
    exit 1
fi

# Check if DMG exists
if [ ! -f "$DMG_FILE" ]; then
    log_info "DMG not found. Building..."
    ./scripts/build-and-sign-macos.sh

    if [ ! -f "$DMG_FILE" ]; then
        log_error "Build failed - DMG not created"
        exit 1
    fi
fi

log_success "Found DMG: $DMG_FILE"

# Check if tag already exists
if git rev-parse "$TAG" >/dev/null 2>&1; then
    log_error "Tag $TAG already exists"
    echo ""
    echo "Options:"
    echo "  1. Delete the tag: git tag -d $TAG && git push origin :refs/tags/$TAG"
    echo "  2. Update version in Cargo.toml"
    exit 1
fi

# Get file size
DMG_SIZE=$(ls -lh "$DMG_FILE" | awk '{print $5}')
log_info "DMG size: $DMG_SIZE"

echo ""
log_info "Creating release notes..."

# Generate release notes
RELEASE_NOTES=$(cat <<EOF
# DJ Uploader ${VERSION}

## Download

Download the DMG installer below and drag DJ Uploader to your Applications folder.

## Installation

1. Download \`DJ-Uploader-${VERSION}.dmg\`
2. Open the DMG file
3. Drag "DJ Uploader.app" to the Applications folder
4. Launch from Applications or Spotlight

## What's New

<!-- Add release notes here -->

## Requirements

- macOS 10.15 (Catalina) or later
- For first launch: Right-click → Open (if unsigned)

---

**File size:** ${DMG_SIZE}
EOF
)

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Release Preview:"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "Tag:     $TAG"
echo "Title:   DJ Uploader v${VERSION}"
echo "File:    $DMG_FILE"
echo ""
echo "Notes:"
echo "$RELEASE_NOTES"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

read -p "Create this release? (y/n) " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    log_info "Release cancelled"
    exit 0
fi

echo ""
log_info "Creating release on GitHub..."

# Create the release
gh release create "$TAG" \
    "$DMG_FILE" \
    --title "DJ Uploader v${VERSION}" \
    --notes "$RELEASE_NOTES" \
    --draft

log_success "Release created as DRAFT!"
echo ""
echo "Next steps:"
echo "  1. Go to: https://github.com/WeekendSuperhero/dj-uploader/releases"
echo "  2. Edit the release to add detailed release notes"
echo "  3. Click 'Publish release' when ready"
echo ""
echo "Or publish immediately with:"
echo "  gh release edit $TAG --draft=false"
echo ""
