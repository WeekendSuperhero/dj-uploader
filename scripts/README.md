# DJ Uploader - macOS Build Scripts

Scripts for building, signing, and packaging DJ Uploader for macOS distribution.

## Quick Start

### Build Without Signing (Local Development)

```bash
./scripts/build-and-sign-macos.sh
```

Creates an unsigned app bundle and DMG. Perfect for local testing.

### Build With Signing (For Distribution)

```bash
# First, get your certificate identity
./scripts/cert-helper.sh list

# Then build with signing
export ENABLE_CODESIGN=true
export CODESIGN_IDENTITY="Apple Development: Your Name (TEAMID)"
./scripts/build-and-sign-macos.sh
```

## Main Scripts

### `build-and-sign-macos.sh`

The primary build script that handles everything:

1. Builds the Rust release binary
2. Creates the macOS .app bundle
3. Code signs the application (optional)
4. Creates a DMG installer
5. Notarizes with Apple (optional)

**Basic usage:**

```bash
./scripts/build-and-sign-macos.sh
```

**With code signing:**

```bash
export ENABLE_CODESIGN=true
export CODESIGN_IDENTITY="Apple Development: mark@weekendsuperhero.io (9NJ8MQVZUN)"
./scripts/build-and-sign-macos.sh
```

**With notarization (requires Developer ID certificate):**

```bash
export ENABLE_CODESIGN=true
export ENABLE_NOTARIZE=true
export CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)"
export APPLE_ID="your@apple.id"
export APPLE_TEAM_ID="TEAMID"
./scripts/build-and-sign-macos.sh
```

### `cert-helper.sh`

All-in-one tool for managing code signing certificates.

**Commands:**

```bash
# List available certificates
./scripts/cert-helper.sh list

# Get setup instructions
./scripts/cert-helper.sh setup

# Diagnose certificate issues
./scripts/cert-helper.sh troubleshoot

# Fix certificate trust issues
./scripts/cert-helper.sh fix
```

### `make_icons.sh`

Generates macOS .icns icon files from source images.

## Environment Variables

| Variable            | Required      | Default              | Description                                       |
| ------------------- | ------------- | -------------------- | ------------------------------------------------- |
| `ENABLE_CODESIGN`   | No            | `false`              | Enable code signing                               |
| `CODESIGN_IDENTITY` | If signing    | -                    | Certificate name (e.g., "Apple Development: ...") |
| `ENABLE_NOTARIZE`   | No            | `false`              | Enable notarization (requires Developer ID)       |
| `APPLE_ID`          | If notarizing | -                    | Your Apple ID email                               |
| `APPLE_TEAM_ID`     | If notarizing | -                    | Your Apple Developer Team ID                      |
| `NOTARY_PROFILE`    | No            | `notarytool-profile` | Keychain profile for notarization credentials     |

## Code Signing Certificates

### Development Certificate (Free)

**Best for:** Local development and testing

- **Cost:** Free with Apple ID
- **Notarization:** Not supported
- **Distribution:** Works locally, may show warnings when shared

**How to get:**

1. Open Xcode
2. Xcode → Preferences → Accounts
3. Add your Apple ID
4. Manage Certificates → + → Apple Development

### Developer ID Certificate ($99/year)

**Best for:** Public distribution outside Mac App Store

- **Cost:** $99/year (Apple Developer Program)
- **Notarization:** Fully supported
- **Distribution:** No warnings for users

**How to get:**

1. Join [Apple Developer Program](https://developer.apple.com/programs/)
2. Create certificate at [developer.apple.com](https://developer.apple.com/account/resources/certificates/list)
3. Select "Developer ID Application"
4. Download and install

## Common Workflows

### Local Testing

```bash
./scripts/build-and-sign-macos.sh
sudo cp -R "DJ Uploader.app" /Applications/
```

### Team Distribution (Signed)

```bash
export ENABLE_CODESIGN=true
export CODESIGN_IDENTITY="Apple Development: Your Name (TEAMID)"
./scripts/build-and-sign-macos.sh

# Share: target/DJ-Uploader-Installer.dmg
```

### Public Release (Signed + Notarized)

```bash
# One-time: Store notarization credentials
xcrun notarytool store-credentials notarytool-profile \
  --apple-id 'your@apple.id' \
  --team-id 'TEAMID'

# Build and notarize
export ENABLE_CODESIGN=true
export ENABLE_NOTARIZE=true
export CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)"
export APPLE_ID="your@apple.id"
export APPLE_TEAM_ID="TEAMID"
./scripts/build-and-sign-macos.sh
```

## Troubleshooting

### "No valid identities found"

**Problem:** No code signing certificates installed or certificates not trusted.

**Solution:**

```bash
./scripts/cert-helper.sh troubleshoot
./scripts/cert-helper.sh fix
```

### "CSSMERR_TP_NOT_TRUSTED"

**Problem:** Certificate exists but system doesn't trust it (missing Apple WWDR certificates).

**Solution:**

```bash
./scripts/cert-helper.sh fix
```

This automatically downloads and installs Apple's intermediate certificates.

### "Developer cannot be verified" warning

**Problem:** App isn't signed or notarized.

**Solution:** Sign your app with a Developer ID certificate and notarize it.

### Certificate has no private key

**Problem:** Certificate was created on another Mac or imported incorrectly.

**Solution:**

- Recreate the certificate in Xcode on this Mac, OR
- Export as .p12 from original Mac and import here

### Icon not found warning

**Problem:** No iconset exists at `assets/dj-uploader.iconset/`

**Solution:** Create icons or ignore (default icon will be used)

## Output Files

After building:

- **App Bundle:** `DJ Uploader.app` (for manual installation)
- **DMG Installer:** `target/DJ-Uploader-Installer.dmg` (for distribution)
- **Entitlements:** `target/entitlements.plist` (if signed)

## Prerequisites

- macOS 10.15 or later
- Xcode Command Line Tools
- Rust and Cargo
- (Optional) Apple Developer account for code signing

## Distribution Checklist

Before releasing your app:

- [ ] App builds successfully
- [ ] App is code signed with Developer ID certificate
- [ ] App is notarized by Apple
- [ ] DMG is code signed
- [ ] Tested on a clean macOS system
- [ ] No security warnings during installation
- [ ] App launches correctly from Applications folder

## Resources

- [Apple Developer Program](https://developer.apple.com/programs/)
- [Code Signing Guide](https://developer.apple.com/support/code-signing/)
- [Notarizing macOS Software](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
- [Customizing Notarization Workflow](https://developer.apple.com/documentation/security/customizing_the_notarization_workflow)

## Archive

Legacy scripts have been moved to `archive/` directory and are no longer needed.
