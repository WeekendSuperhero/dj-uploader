#!/bin/bash
set -e

APP_NAME="DJ Uploader"
BUNDLE_ID="com.djuploader.app"
VERSION="0.1.0"
BINARY_NAME="dj-uploader"

echo "Building macOS app bundle for ${APP_NAME}..."

# Build release binary
echo "Building release binary..."
cargo build --release

# Create app bundle structure
APP_DIR="${APP_NAME}.app"
CONTENTS_DIR="${APP_DIR}/Contents"
MACOS_DIR="${CONTENTS_DIR}/MacOS"
RESOURCES_DIR="${CONTENTS_DIR}/Resources"

echo "Creating bundle structure..."
rm -rf "${APP_DIR}"
mkdir -p "${MACOS_DIR}"
mkdir -p "${RESOURCES_DIR}"

# Copy binary
echo "Copying binary..."
cp "target/release/${BINARY_NAME}" "${MACOS_DIR}/"

# Create Info.plist
echo "Creating Info.plist..."
cat > "${CONTENTS_DIR}/Info.plist" << EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>${BINARY_NAME}</string>
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

# Create launcher script that always runs in GUI mode
echo "Creating launcher script..."
cat > "${MACOS_DIR}/${BINARY_NAME}-launcher" << 'EOF'
#!/bin/bash
DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
"${DIR}/dj-uploader" --gui
EOF

chmod +x "${MACOS_DIR}/${BINARY_NAME}-launcher"

# Update Info.plist to use launcher
sed -i '' "s|<string>${BINARY_NAME}</string>|<string>${BINARY_NAME}-launcher</string>|g" "${CONTENTS_DIR}/Info.plist"

# Create icon from iconset if available
echo "Processing app icon..."
if [ -d "assets/dj-uploader.iconset" ]; then
    echo "Converting iconset to .icns..."
    iconutil -c icns "assets/dj-uploader.iconset" -o "${RESOURCES_DIR}/AppIcon.icns"
    echo "✓ Icon created from assets/dj-uploader.iconset"
else
    echo "⚠ No iconset found at assets/dj-uploader.iconset"
    echo "  App will use default icon. To add a custom icon:"
    echo "  1. Create assets/dj-uploader.iconset/ with required icon sizes"
    echo "  2. Re-run this script"
fi

echo ""
echo "✓ App bundle created: ${APP_DIR}"
echo ""
echo "To install:"
echo "  sudo cp -R '${APP_DIR}' /Applications/"
echo ""
echo "To create installer DMG:"
echo "  ./create-dmg.sh"
echo ""
