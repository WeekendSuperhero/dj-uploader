#!/bin/bash

# Script to create a DMG installer for DJ Uploader
# This creates a professional macOS installer with drag-to-Applications functionality

set -e

APP_NAME="DJ Uploader"
APP_BUNDLE="DJ Uploader.app"
DMG_NAME="DJ-Uploader-Installer.dmg"
VOLUME_NAME="DJ Uploader Installer"
BUNDLE_DIR="./"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Creating DMG installer for ${APP_NAME}...${NC}"

# Check if bundle exists
if [ ! -d "${BUNDLE_DIR}/${APP_BUNDLE}" ]; then
    echo "Error: App bundle not found. Run ./bundle-macos.sh first."
    exit 1
fi

# Create temporary directory for DMG contents
DMG_TMP="target/dmg-temp"
rm -rf "${DMG_TMP}"
mkdir -p "${DMG_TMP}"

echo "Copying app bundle to temporary directory..."
cp -R "${BUNDLE_DIR}/${APP_BUNDLE}" "${DMG_TMP}/"

# Create Applications symlink for drag-and-drop installation
echo "Creating Applications symlink..."
ln -s /Applications "${DMG_TMP}/Applications"

# Remove old DMG if it exists
rm -f "target/${DMG_NAME}"

echo "Creating DMG..."

# Create the DMG
hdiutil create -volname "${VOLUME_NAME}" \
    -srcfolder "${DMG_TMP}" \
    -ov \
    -format UDZO \
    "target/${DMG_NAME}"

# Clean up temporary directory
rm -rf "${DMG_TMP}"

echo -e "${GREEN}✓ DMG created successfully!${NC}"
echo ""
echo "DMG location: target/${DMG_NAME}"
echo ""
echo "To distribute your app:"
echo "1. Share the DMG file with users"
echo "2. Users open the DMG and drag '${APP_BUNDLE}' to Applications"
echo "3. Users can then launch from Applications or Spotlight"
echo ""
echo "The DMG is ready for distribution!"
