#!/bin/bash
set -e

# Build script for universal macOS binary (Apple Silicon + Intel)
#
# For code signing and notarization, set these environment variables:
#   APPLE_SIGNING_IDENTITY - Your Developer ID Application certificate name
#                            (e.g., "Developer ID Application: Your Name (TEAMID)")
#   APPLE_ID               - Your Apple ID email
#   APPLE_TEAM_ID          - Your 10-character team ID
#   APPLE_APP_PASSWORD     - App-specific password for notarization
#
# To find your signing identity:
#   security find-identity -v -p codesigning
#
# To create an app-specific password:
#   https://appleid.apple.com → Sign-In and Security → App-Specific Passwords

cd "$(dirname "$0")"

# Ensure we use rustup's toolchain
export PATH="$HOME/.cargo/bin:$PATH"

# Check for code signing setup
SIGN_APP=false
if [ -n "$APPLE_SIGNING_IDENTITY" ]; then
    echo "Code signing enabled with identity: $APPLE_SIGNING_IDENTITY"
    SIGN_APP=true

    if [ -z "$APPLE_ID" ] || [ -z "$APPLE_TEAM_ID" ] || [ -z "$APPLE_APP_PASSWORD" ]; then
        echo "Warning: Notarization credentials not fully set. App will be signed but not notarized."
        echo "Set APPLE_ID, APPLE_TEAM_ID, and APPLE_APP_PASSWORD for notarization."
        NOTARIZE=false
    else
        NOTARIZE=true
    fi
else
    echo "Warning: APPLE_SIGNING_IDENTITY not set. App will be unsigned."
    echo "Users will see Gatekeeper warnings when opening the app."
fi

echo "=== Building FilingExplorer MCP for macOS (Universal) ==="

# Build mcp-server for both architectures
echo ""
echo "Building mcp-server for Apple Silicon (aarch64)..."
cargo build --release --bin mcp-server --target aarch64-apple-darwin

echo ""
echo "Building mcp-server for Intel (x86_64)..."
cargo build --release --bin mcp-server --target x86_64-apple-darwin

# Create universal binary using lipo
echo ""
echo "Creating universal mcp-server binary..."
mkdir -p target/universal-apple-darwin/release
lipo -create \
  target/aarch64-apple-darwin/release/mcp-server \
  target/x86_64-apple-darwin/release/mcp-server \
  -output target/universal-apple-darwin/release/mcp-server

# Verify universal binary
echo ""
echo "Verifying universal binary:"
file target/universal-apple-darwin/release/mcp-server
lipo -info target/universal-apple-darwin/release/mcp-server

# Copy with target triple suffixes for Tauri sidecar
echo ""
echo "Preparing binaries for Tauri bundling..."
cp target/aarch64-apple-darwin/release/mcp-server target/release/mcp-server-aarch64-apple-darwin
cp target/x86_64-apple-darwin/release/mcp-server target/release/mcp-server-x86_64-apple-darwin

# Build the Tauri app for both architectures
echo ""
echo "Building Settings app for Apple Silicon..."
cd crates/settings-app
cargo tauri build --target aarch64-apple-darwin

echo ""
echo "Building Settings app for Intel..."
cargo tauri build --target x86_64-apple-darwin

cd ../..

# Create universal app bundle
echo ""
echo "Creating universal app bundle..."
AARCH64_APP="target/aarch64-apple-darwin/release/bundle/macos/FilingExplorer Settings.app"
X86_64_APP="target/x86_64-apple-darwin/release/bundle/macos/FilingExplorer Settings.app"
UNIVERSAL_APP="target/universal-apple-darwin/release/bundle/macos/FilingExplorer Settings.app"

mkdir -p "$(dirname "$UNIVERSAL_APP")"
rm -rf "$UNIVERSAL_APP"
cp -R "$AARCH64_APP" "$UNIVERSAL_APP"

# Create universal binaries for main app and mcp-server inside the bundle
lipo -create \
  "$AARCH64_APP/Contents/MacOS/filing-explorer-settings" \
  "$X86_64_APP/Contents/MacOS/filing-explorer-settings" \
  -output "$UNIVERSAL_APP/Contents/MacOS/filing-explorer-settings"

lipo -create \
  "$AARCH64_APP/Contents/MacOS/mcp-server" \
  "$X86_64_APP/Contents/MacOS/mcp-server" \
  -output "$UNIVERSAL_APP/Contents/MacOS/mcp-server"

echo ""
echo "Verifying universal app binaries:"
file "$UNIVERSAL_APP/Contents/MacOS/filing-explorer-settings"
file "$UNIVERSAL_APP/Contents/MacOS/mcp-server"

# Code signing
if [ "$SIGN_APP" = true ]; then
    echo ""
    echo "=== Code Signing ==="

    # Sign the mcp-server binary first (it's embedded)
    echo "Signing mcp-server..."
    codesign --force --options runtime --timestamp \
        --sign "$APPLE_SIGNING_IDENTITY" \
        "$UNIVERSAL_APP/Contents/MacOS/mcp-server"

    # Sign any frameworks/libraries (if present)
    if [ -d "$UNIVERSAL_APP/Contents/Frameworks" ]; then
        echo "Signing frameworks..."
        find "$UNIVERSAL_APP/Contents/Frameworks" -type f -perm +111 | while read -r binary; do
            codesign --force --options runtime --timestamp \
                --sign "$APPLE_SIGNING_IDENTITY" \
                "$binary"
        done
    fi

    # Sign the main app binary
    echo "Signing main app..."
    codesign --force --options runtime --timestamp \
        --sign "$APPLE_SIGNING_IDENTITY" \
        "$UNIVERSAL_APP/Contents/MacOS/filing-explorer-settings"

    # Sign the entire bundle
    echo "Signing app bundle..."
    codesign --force --options runtime --timestamp \
        --sign "$APPLE_SIGNING_IDENTITY" \
        "$UNIVERSAL_APP"

    # Verify signature
    echo "Verifying signature..."
    codesign --verify --deep --strict --verbose=2 "$UNIVERSAL_APP"

    echo "Code signing complete."
fi

# Create DMG for universal build
echo ""
echo "Creating DMG..."
DMG_DIR="target/universal-apple-darwin/release/bundle/dmg"
mkdir -p "$DMG_DIR"
DMG_PATH="$DMG_DIR/FilingExplorer Settings_0.1.0_universal.dmg"
rm -f "$DMG_PATH"

hdiutil create -volname "FilingExplorer Settings" \
  -srcfolder "$UNIVERSAL_APP" \
  -ov -format UDZO \
  "$DMG_PATH"

# Sign and notarize DMG
if [ "$SIGN_APP" = true ]; then
    echo ""
    echo "Signing DMG..."
    codesign --force --timestamp \
        --sign "$APPLE_SIGNING_IDENTITY" \
        "$DMG_PATH"

    if [ "$NOTARIZE" = true ]; then
        echo ""
        echo "=== Notarizing ==="
        echo "Submitting to Apple for notarization..."

        # Submit for notarization
        xcrun notarytool submit "$DMG_PATH" \
            --apple-id "$APPLE_ID" \
            --team-id "$APPLE_TEAM_ID" \
            --password "$APPLE_APP_PASSWORD" \
            --wait

        # Staple the notarization ticket
        echo "Stapling notarization ticket..."
        xcrun stapler staple "$DMG_PATH"

        echo "Notarization complete."
    fi
fi

echo ""
echo "=== Build Complete ==="
echo ""
echo "Universal app bundle:"
echo "  $UNIVERSAL_APP"
echo ""
echo "Universal DMG:"
echo "  $DMG_PATH"
echo ""
echo "Supported architectures: Apple Silicon (M1/M2/M3) + Intel (x86_64)"

if [ "$SIGN_APP" = true ]; then
    echo ""
    echo "Status: Signed"
    if [ "$NOTARIZE" = true ]; then
        echo "Status: Notarized (no Gatekeeper warnings)"
    else
        echo "Status: NOT notarized (Gatekeeper may warn on first launch)"
    fi
else
    echo ""
    echo "Status: Unsigned"
    echo "Warning: Users will see Gatekeeper security warnings."
    echo "To sign, set APPLE_SIGNING_IDENTITY environment variable."
fi
