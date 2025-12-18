#!/bin/bash

# Certificate Helper - All-in-one tool for managing code signing certificates
# This tool helps you list, troubleshoot, and fix certificate issues

set -e

GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BOLD='\033[1m'
NC='\033[0m'

print_header() {
    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BLUE}${BOLD}    $1${NC}"
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
}

print_section() {
    echo ""
    echo -e "${BOLD}$1${NC}"
    echo ""
}

cmd_list() {
    print_header "Available Code Signing Certificates"

    echo -e "${BLUE}All Code Signing Identities:${NC}"
    echo ""

    IDENTITIES=$(security find-identity -v -p codesigning 2>&1)

    if echo "$IDENTITIES" | grep -q "0 valid identities found"; then
        echo -e "${RED}✗ No valid code signing certificates found${NC}"
        echo ""
        echo "Run: ${YELLOW}$0 setup${NC} for help getting started"
        return 1
    else
        echo "$IDENTITIES"
    fi

    echo ""
    echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    print_section "How to Use"

    echo "1. Copy the identity name from above (everything in quotes)"
    echo "2. Set environment variables:"
    echo ""
    echo -e "   ${YELLOW}export ENABLE_CODESIGN=true${NC}"
    echo -e "   ${YELLOW}export CODESIGN_IDENTITY='Apple Development: Your Name (TEAMID)'${NC}"
    echo ""
    echo "3. Build your app:"
    echo -e "   ${YELLOW}./scripts/build-and-sign-macos.sh${NC}"
    echo ""

    print_section "Certificate Types"

    echo -e "${GREEN}Developer ID Application:${NC}"
    echo "  • For distribution outside Mac App Store"
    echo "  • Requires \$99/year Apple Developer account"
    echo "  • Can be notarized"
    echo "  • Best for: Public distribution"
    echo ""
    echo -e "${GREEN}Apple Development / Mac Development:${NC}"
    echo "  • For development and testing"
    echo "  • Free with Apple ID"
    echo "  • Cannot be notarized"
    echo "  • Best for: Local testing, team sharing"
    echo ""
}

cmd_troubleshoot() {
    print_header "Certificate Troubleshooting Diagnostic"

    print_section "1. Checking Keychains"
    security list-keychains

    print_section "2. Code Signing Certificates"
    CODESIGN_CERTS=$(security find-identity -v -p codesigning 2>&1)
    echo "$CODESIGN_CERTS"
    echo ""

    if echo "$CODESIGN_CERTS" | grep -q "0 valid identities found"; then
        echo -e "${RED}✗ No valid code signing certificates found${NC}"
        FOUND_UNTRUSTED=false

        # Check for untrusted certificates
        print_section "3. Checking for Untrusted Certificates"
        ALL_CERTS=$(security find-identity 2>&1)

        if echo "$ALL_CERTS" | grep -q "CSSMERR_TP_NOT_TRUSTED"; then
            echo -e "${YELLOW}⚠ Found untrusted certificates!${NC}"
            echo ""
            echo "$ALL_CERTS" | grep "Apple Development\|Developer ID"
            echo ""
            FOUND_UNTRUSTED=true
        fi

        echo ""
        echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        print_section "Diagnosis"

        if [ "$FOUND_UNTRUSTED" = true ]; then
            echo -e "${YELLOW}Issue: Your certificates are not trusted by the system${NC}"
            echo ""
            echo "Common causes:"
            echo "  • Missing Apple WWDR intermediate certificates"
            echo "  • Certificate doesn't have private key"
            echo "  • Certificate needs manual trust configuration"
            echo ""
            echo "Solution:"
            echo -e "  Run: ${YELLOW}$0 fix${NC} to automatically fix trust issues"
        else
            echo -e "${YELLOW}Issue: No code signing certificates found${NC}"
            echo ""
            echo "Solution:"
            echo -e "  Run: ${YELLOW}$0 setup${NC} to learn how to get certificates"
        fi
    else
        echo -e "${GREEN}✓ Certificates look good!${NC}"
        echo ""
        echo "You're ready to sign your app. Run:"
        echo -e "  ${YELLOW}$0 list${NC} to see your certificates"
    fi

    echo ""
}

cmd_fix() {
    print_header "Fixing Certificate Trust Issues"

    # Check for Apple Development certificates
    print_section "Checking for Certificates"

    ALL_CERTS=$(security find-identity 2>&1)

    if ! echo "$ALL_CERTS" | grep -q "Apple Development\|Developer ID"; then
        echo -e "${RED}✗ No Apple certificates found${NC}"
        echo ""
        echo "Please create certificates first:"
        echo -e "  Run: ${YELLOW}$0 setup${NC} for instructions"
        exit 1
    fi

    if echo "$ALL_CERTS" | grep -q "CSSMERR_TP_NOT_TRUSTED"; then
        echo -e "${YELLOW}⚠ Found untrusted certificates${NC}"
        echo ""
        echo "$ALL_CERTS" | grep "Apple Development\|Developer ID"
        echo ""
    else
        echo -e "${GREEN}✓ All certificates are already trusted${NC}"
        echo ""
        exit 0
    fi

    print_section "Installing Apple WWDR Certificates"

    echo "Downloading Apple Worldwide Developer Relations certificates..."
    echo "These intermediate certificates are required for trust validation."
    echo ""

    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"

    for cert in AppleWWDRCAG3 AppleWWDRCAG4 AppleWWDRCAG5 AppleWWDRCAG6; do
        echo "  • Downloading ${cert}..."
        curl -sO "https://www.apple.com/certificateauthority/${cert}.cer" || true
    done

    echo ""
    echo "Installing certificates (requires sudo)..."

    for cert in *.cer; do
        if [ -f "$cert" ]; then
            echo "  • Installing ${cert}..."
            sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain "$cert" 2>/dev/null || true
        fi
    done

    cd - > /dev/null
    rm -rf "$TEMP_DIR"

    echo ""
    echo -e "${GREEN}✓ WWDR certificates installed${NC}"

    print_section "Verifying Certificates"

    VALID_CERTS=$(security find-identity -v -p codesigning 2>&1)

    if echo "$VALID_CERTS" | grep -q "Apple Development\|Developer ID" && ! echo "$VALID_CERTS" | grep -q "0 valid"; then
        echo -e "${GREEN}✓✓✓ SUCCESS! Your certificates are now trusted! ✓✓✓${NC}"
        echo ""
        echo "$VALID_CERTS"
        echo ""
        echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        echo -e "${GREEN}${BOLD}    Ready to Sign!${NC}"
        echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        echo ""
        echo "Use one of the identities above:"
        echo ""
        echo -e "${YELLOW}export ENABLE_CODESIGN=true${NC}"
        echo -e "${YELLOW}export CODESIGN_IDENTITY=\"<identity from above>\"${NC}"
        echo -e "${YELLOW}./scripts/build-and-sign-macos.sh${NC}"
        echo ""
    else
        echo -e "${YELLOW}⚠ Certificates still showing as untrusted${NC}"
        echo ""
        echo "Additional manual steps needed:"
        echo ""
        echo "1. Open Keychain Access"
        echo "2. Select 'login' keychain"
        echo "3. Find your Apple Development certificate"
        echo "4. Click the ▶ arrow next to it"
        echo ""
        echo "If you DON'T see a private key underneath:"
        echo "  → Recreate the certificate in Xcode on THIS Mac"
        echo "  → Xcode → Preferences → Accounts → Manage Certificates"
        echo ""
        echo "If you DO see a private key:"
        echo "  → Double-click the certificate"
        echo "  → Expand 'Trust'"
        echo "  → Set 'Code Signing' to 'Always Trust'"
        echo ""
    fi
}

cmd_setup() {
    print_header "Code Signing Setup Guide"

    print_section "Option 1: Free Development Certificate (Recommended for Testing)"

    echo "Get a free Apple Development certificate through Xcode:"
    echo ""
    echo "1. Open Xcode"
    echo "2. Xcode → Preferences → Accounts"
    echo "3. Click '+' and add your Apple ID (free)"
    echo "4. Select your account → Manage Certificates"
    echo "5. Click '+' → Apple Development"
    echo ""
    echo -e "${GREEN}✓ Free${NC}"
    echo -e "${GREEN}✓ Good for development and testing${NC}"
    echo -e "${YELLOW}✗ Cannot be notarized (may show warnings when shared)${NC}"
    echo ""

    print_section "Option 2: Developer ID Certificate (For Public Distribution)"

    echo "Get a Developer ID Application certificate for distribution:"
    echo ""
    echo "1. Join Apple Developer Program (\$99/year)"
    echo "   → https://developer.apple.com/programs/"
    echo ""
    echo "2. Create certificate"
    echo "   → https://developer.apple.com/account/resources/certificates/list"
    echo "   → Click '+' → Select 'Developer ID Application'"
    echo ""
    echo "3. Download and install the certificate"
    echo "   → Double-click the downloaded .cer file"
    echo ""
    echo -e "${YELLOW}✗ Costs \$99/year${NC}"
    echo -e "${GREEN}✓ Can be notarized (no warnings for users)${NC}"
    echo -e "${GREEN}✓ Best for public distribution${NC}"
    echo ""

    print_section "After Getting a Certificate"

    echo "1. Verify it's installed:"
    echo -e "   ${YELLOW}$0 list${NC}"
    echo ""
    echo "2. If you see trust errors:"
    echo -e "   ${YELLOW}$0 fix${NC}"
    echo ""
    echo "3. Build your signed app:"
    echo -e "   ${YELLOW}export ENABLE_CODESIGN=true${NC}"
    echo -e "   ${YELLOW}export CODESIGN_IDENTITY='<your identity>'${NC}"
    echo -e "   ${YELLOW}./scripts/build-and-sign-macos.sh${NC}"
    echo ""
}

cmd_help() {
    print_header "Certificate Helper - Usage"

    echo "This tool helps you manage code signing certificates for macOS."
    echo ""
    echo -e "${BOLD}Commands:${NC}"
    echo ""
    echo -e "  ${YELLOW}$0 list${NC}"
    echo "      List all available code signing certificates"
    echo ""
    echo -e "  ${YELLOW}$0 setup${NC}"
    echo "      Show how to get code signing certificates"
    echo ""
    echo -e "  ${YELLOW}$0 troubleshoot${NC}"
    echo "      Diagnose certificate issues"
    echo ""
    echo -e "  ${YELLOW}$0 fix${NC}"
    echo "      Automatically fix certificate trust issues"
    echo ""
    echo -e "  ${YELLOW}$0 help${NC}"
    echo "      Show this help message"
    echo ""
    echo -e "${BOLD}Quick Start:${NC}"
    echo ""
    echo "  First time? Run:  ${YELLOW}$0 setup${NC}"
    echo "  Have certs? Run:  ${YELLOW}$0 list${NC}"
    echo "  Having issues?:   ${YELLOW}$0 troubleshoot${NC}"
    echo ""
}

# Main command dispatcher
case "${1:-help}" in
    list|l)
        cmd_list
        ;;
    troubleshoot|t|debug)
        cmd_troubleshoot
        ;;
    fix|f|repair)
        cmd_fix
        ;;
    setup|s|install)
        cmd_setup
        ;;
    help|h|-h|--help)
        cmd_help
        ;;
    *)
        echo -e "${RED}Unknown command: $1${NC}"
        echo ""
        cmd_help
        exit 1
        ;;
esac
