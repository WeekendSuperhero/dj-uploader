#!/bin/bash

# Unified script to build, sign, and package DJ Uploader for macOS
# This script:
# 1. Builds the release binary
# 2. Creates the .app bundle
# 3. Code signs the application
# 4. Creates a DMG installer
# 5. Optionally notarizes the DMG for distribution

set -e

# Parse command line arguments
DRY_RUN=false
for arg in "$@"; do
    case $arg in
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        *)
            # Unknown option
            ;;
    esac
done

if [ "$DRY_RUN" = true ]; then
    echo "Running in dry-run mode. No actual building or signing will occur."
fi

# Configuration
APP_NAME="DJ Uploader"
BUNDLE_ID="com.djuploader.app"
BINARY_NAME="dj-uploader"
APP_BUNDLE="${APP_NAME}.app"
VOLUME_NAME="DJ Uploader Installer"

# Read version from Cargo.toml
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
if [ -f "$PROJECT_ROOT/Cargo.toml" ]; then
    VERSION=$(grep -E '^version = ' "$PROJECT_ROOT/Cargo.toml" | head -1 | sed -E 's/version = "(.*)"/\1/')
else
    VERSION="0.1.0"
    echo "Warning: Could not find Cargo.toml, using default version $VERSION"
fi

# DMG name includes version
DMG_NAME="DJ-Uploader-${VERSION}.dmg"

# Code signing configuration
# Set these via environment variables or edit here
SIGNING_IDENTITY="${CODESIGN_IDENTITY:-}"
ENABLE_SIGNING="${ENABLE_CODESIGN:-false}"
ENABLE_NOTARIZATION="${ENABLE_NOTARIZE:-false}"
NOTARY_PROFILE="${NOTARY_PROFILE:-notarytool-profile}"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Logging functions
log_info() {
    echo -e "${BLUE}ℹ ${1}${NC}"
}

log_success() {
    echo -e "${GREEN}✓ ${1}${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠ ${1}${NC}"
}

log_error() {
    echo -e "${RED}✗ ${1}${NC}"
}

progress_bar() {
    local duration=${1}
    local interval=0.1
    local count=$(echo "$duration / $interval" | bc -l)

    for ((i=0; i<count; i++)); do
        printf "⏳"
        sleep $interval
    done
    printf "\n"
}

# Function to check if required tools exist
check_prerequisites() {
    local missing_tools=()

    command -v cargo >/dev/null 2>&1 || missing_tools+=("cargo")
    command -v iconutil >/dev/null 2>&1 || missing_tools+=("iconutil")
    command -v hdiutil >/dev/null 2>&1 || missing_tools+=("hdiutil")
    command -v codesign >/dev/null 2>&1 || missing_tools+=("codesign")
    command -v security >/dev/null 2>&1 || missing_tools+=("security")

    if [ ${#missing_tools[@]} -ne 0 ]; then
        log_error "Missing required tools: ${missing_tools[*]}"
        exit 1
    fi
}

# Function to check if signing identity exists
check_signing_identity() {
    if [ -z "$SIGNING_IDENTITY" ]; then
        return 1
    fi

    if security find-identity -v -p codesigning | grep -q "$SIGNING_IDENTITY"; then
        return 0
    else
        return 1
    fi
}

# Function to list available signing identities
list_signing_identities() {
    log_info "Available code signing identities:"
    security find-identity -v -p codesigning || echo "  None found"
    echo ""
    log_info "Tip: Run ./scripts/list-certificates.sh for more detailed information"
}

# Function to select signing identity interactively
select_signing_identity() {
    log_info "Looking for available signing identities..."
    local identities=($(security find-identity -v -p codesigning | grep "Developer ID Application" | cut -d'"' -f2))

    if [ ${#identities[@]} -eq 0 ]; then
        log_error "No Developer ID Application identities found."
        list_signing_identities
        return 1
    elif [ ${#identities[@]} -eq 1 ]; then
        SIGNING_IDENTITY="${identities[0]}"
        log_success "Found one identity: $SIGNING_IDENTITY"
    else
        log_info "Multiple signing identities found:"
        for i in "${!identities[@]}"; do
            echo "  [$((i+1))] ${identities[i]}"
        done

        echo ""
        while true; do
            read -p "Select identity [1-${#identities[@]}]: " choice
            if [[ $choice =~ ^[1-${#identities[@]}]$ ]]; then
                SIGNING_IDENTITY="${identities[$((choice-1))]}"
                log_success "Selected identity: $SIGNING_IDENTITY"
                break
            else
                log_error "Invalid selection. Please enter a number between 1 and ${#identities[@]}."
            fi
        done
    fi

    export CODESIGN_IDENTITY="$SIGNING_IDENTITY"
}

# Validate project structure
validate_project() {
    if [ ! -f "$PROJECT_ROOT/Cargo.toml" ]; then
        log_error "Cargo.toml not found in project root: $PROJECT_ROOT"
        exit 1
    fi

    if [ ! -f "$PROJECT_ROOT/src/main.rs" ]; then
        log_error "Main Rust source file not found: $PROJECT_ROOT/src/main.rs"
        exit 1
    fi

    if [ ! -d "$PROJECT_ROOT/assets" ]; then
        log_warning "Assets directory not found: $PROJECT_ROOT/assets"
    fi
}

# Print banner
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
log_info "DJ Uploader macOS Build & Packaging Script"
log_info "Version: ${VERSION}"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Validate prerequisites
log_info "Validating prerequisites..."
check_prerequisites
validate_project
log_success "Prerequisites validated"
echo ""

# Check code signing setup
if [ "$ENABLE_SIGNING" = "true" ]; then
    log_info "Code signing: ENABLED"
    if [ -z "$SIGNING_IDENTITY" ]; then
        log_info "No signing identity specified, attempting to select one..."
        select_signing_identity || {
            log_error "Could not select a signing identity"
            exit 1
        }
    fi

    if check_signing_identity; then
        log_success "Signing identity found: $SIGNING_IDENTITY"
    else
        log_error "Signing identity not found: $SIGNING_IDENTITY"
        list_signing_identities
        echo ""
        log_error "Please set CODESIGN_IDENTITY to a valid identity or disable signing"
        exit 1
    fi
else
    log_warning "Code signing: DISABLED"
    log_info "To enable code signing, set: export ENABLE_CODESIGN=true"
    log_info "And set your identity: export CODESIGN_IDENTITY='Developer ID Application: Your Name (TEAMID)'"
    echo ""
fi

# ============================================================================
# STEP 1: Build Release Binary
# ============================================================================
log_info "Step 1/5: Building release binary..."
cargo build --release
log_success "Binary built successfully"
echo ""

# ============================================================================
# STEP 2: Create App Bundle Structure
# ============================================================================
log_info "Step 2/5: Creating app bundle..."

CONTENTS_DIR="${APP_BUNDLE}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"

# Clean and create directories
rm -rf "${APP_BUNDLE}"
mkdir -p "${MACOS_DIR}"
mkdir -p "${RESOURCES_DIR}"

# Copy binary
cp "target/release/${BINARY_NAME}" "${MACOS_DIR}/"

# Create Info.plist
cat > "${CONTENTS_DIR}/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>${BINARY_NAME}-launcher</string>
    <key>CFBundleIdentifier</key>
    <string>${BUNDLE_ID}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>${APP_NAME}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>${VERSION}</string>
    <key>CFBundleVersion</key>
    <string>${VERSION}</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeExtensions</key>
            <array>
                <string>mp3</string>
                <string>m4a</string>
                <string>wav</string>
                <string>flac</string>
            </array>
            <key>CFBundleTypeName</key>
            <string>Audio File</string>
            <key>CFBundleTypeRole</key>
            <string>Viewer</string>
        </dict>
    </array>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.music</string>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © 2024</string>
</dict>
</plist>
EOF

# Create launcher script
cat > "${MACOS_DIR}/${BINARY_NAME}-launcher" << 'LAUNCHER_EOF'
#!/bin/bash
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
"${DIR}/dj-uploader" --gui
LAUNCHER_EOF

chmod +x "${MACOS_DIR}/${BINARY_NAME}-launcher"

# Process icon
if [ -d "assets/dj-uploader.iconset" ]; then
    iconutil -c icns "assets/dj-uploader.iconset" -o "${RESOURCES_DIR}/AppIcon.icns"
    log_success "Icon created from assets/dj-uploader.iconset"
else
    log_warning "No iconset found at assets/dj-uploader.iconset - using default icon"
fi

log_success "App bundle created: ${APP_BUNDLE}"
echo ""

# ============================================================================
# STEP 3: Code Sign the App Bundle
# ============================================================================
if [ "$ENABLE_SIGNING" = "true" ]; then
    log_info "Step 3/5: Code signing the application..."

    # Create entitlements file
    ENTITLEMENTS_FILE="target/entitlements.plist"
    cat > "$ENTITLEMENTS_FILE" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
    <key>com.apple.security.cs.allow-jit</key>
    <true/>
    <key>com.apple.security.network.client</key>
    <true/>
    <key>com.apple.security.files.user-selected.read-write</key>
    <true/>
</dict>
</plist>
EOF

    # Sign the binary first
    log_info "Signing binary..."
    codesign --force --options runtime \
        --entitlements "$ENTITLEMENTS_FILE" \
        --sign "$SIGNING_IDENTITY" \
        --timestamp \
        "${MACOS_DIR}/${BINARY_NAME}"

    # Sign the launcher script
    log_info "Signing launcher..."
    codesign --force --options runtime \
        --sign "$SIGNING_IDENTITY" \
        --timestamp \
        "${MACOS_DIR}/${BINARY_NAME}-launcher"

    # Sign the entire app bundle
    log_info "Signing app bundle..."
    codesign --force --options runtime \
        --entitlements "$ENTITLEMENTS_FILE" \
        --sign "$SIGNING_IDENTITY" \
        --timestamp \
        --deep \
        "${APP_BUNDLE}"

    # Verify the signature
    log_info "Verifying signature..."
    codesign --verify --deep --strict --verbose=2 "${APP_BUNDLE}"

    log_success "Application signed successfully"
    echo ""
else
    log_warning "Step 3/5: Skipping code signing (disabled)"
    echo ""
fi

# ============================================================================
# STEP 4: Create DMG Installer
# ============================================================================
log_info "Step 4/5: Creating DMG installer..."

DMG_TMP="target/dmg-temp"
rm -rf "${DMG_TMP}"
mkdir -p "${DMG_TMP}"

# Copy app bundle to temporary directory
cp -R "${APP_BUNDLE}" "${DMG_TMP}/"

# Create Applications symlink
ln -s /Applications "${DMG_TMP}/Applications"

# Remove old DMG if it exists
rm -f "target/${DMG_NAME}"

# Create the DMG
hdiutil create -volname "${VOLUME_NAME}" \
    -srcfolder "${DMG_TMP}" \
    -ov \
    -format UDZO \
    "target/${DMG_NAME}"

# Clean up temporary directory
rm -rf "${DMG_TMP}"

log_success "DMG created: target/${DMG_NAME}"
echo ""

# Sign the DMG if code signing is enabled
if [ "$ENABLE_SIGNING" = "true" ]; then
    log_info "Signing DMG..."
    codesign --force \
        --sign "$SIGNING_IDENTITY" \
        --timestamp \
        "target/${DMG_NAME}"

    log_success "DMG signed successfully"
    echo ""
fi

# ============================================================================
# STEP 5: Notarization (Optional)
# ============================================================================
if [ "$ENABLE_NOTARIZATION" = "true" ] && [ "$ENABLE_SIGNING" = "true" ]; then
    log_info "Step 5/5: Notarizing DMG with Apple..."

    log_info "Submitting to Apple notary service..."
    log_info "This may take several minutes..."

    # Submit for notarization
    xcrun notarytool submit "target/${DMG_NAME}" \
        --keychain-profile "$NOTARY_PROFILE" \
        --wait

    # Staple the notarization ticket to the DMG
    log_info "Stapling notarization ticket..."
    xcrun stapler staple "target/${DMG_NAME}"

    log_success "Notarization complete!"
    echo ""
else
    log_warning "Step 5/5: Skipping notarization"
    if [ "$ENABLE_SIGNING" = "true" ]; then
        log_info "To enable notarization:"
        log_info "  1. Store credentials: xcrun notarytool store-credentials $NOTARY_PROFILE"
        log_info "  2. Set environment variables:"
        log_info "     export ENABLE_NOTARIZE=true"
    fi
    echo ""
fi

# ============================================================================
# Summary
# ============================================================================
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
log_success "Build Complete!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "📦 App Bundle:  ${APP_BUNDLE}"
echo "💿 DMG File:    target/${DMG_NAME}"
echo ""

if [ "$ENABLE_SIGNING" = "true" ]; then
    echo "✅ Code Signed: YES"
    if [ "$ENABLE_NOTARIZATION" = "true" ]; then
        echo "✅ Notarized:   YES"
        echo ""
        echo "Your app is ready for distribution!"
    else
        echo "⚠️  Notarized:   NO"
        echo ""
        echo "Note: For distribution outside the Mac App Store, notarization is recommended."
    fi
else
    echo "⚠️  Code Signed: NO"
    echo "⚠️  Notarized:   NO"
    echo ""
    echo "Note: Unsigned apps will show security warnings when users try to open them."
fi

echo ""
echo "To install locally:"
echo "  sudo cp -R '${APP_BUNDLE}' /Applications/"
echo ""
echo "To distribute:"
echo "  Share target/${DMG_NAME} with users"
echo ""
