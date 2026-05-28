#!/usr/bin/env bash
# Added: TMAIL-56 — local release helper.
#
# Builds a signed Android App Bundle (AAB) + universal APK for TASMail Mobile.
# Versioning is driven by CLI args so the same script works for fastlane,
# CI, and ad-hoc operator builds.
#
# Usage:
#   mobile/scripts/release.sh <build-name> <build-number>
#
# Example:
#   mobile/scripts/release.sh 1.2.0 17
#
# Outputs:
#   mobile/build/app/outputs/bundle/release/app-release.aab
#   mobile/build/app/outputs/flutter-apk/app-release.apk
#
# Prereqs:
#   - mobile/android/key.properties present + populated
#   - mobile/android/app/<keystore>.jks present at the path key.properties points at
#   - flutter on $PATH

set -euo pipefail

BUILD_NAME="${1:-}"
BUILD_NUMBER="${2:-}"

if [[ -z "$BUILD_NAME" || -z "$BUILD_NUMBER" ]]; then
    echo "Usage: $0 <build-name> <build-number>" >&2
    echo "Example: $0 1.2.0 17" >&2
    exit 1
fi

# Resolve repo paths regardless of where the script was invoked from.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MOBILE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "$MOBILE_DIR"

if [[ ! -f "android/key.properties" ]]; then
    echo "❌ android/key.properties not found." >&2
    echo "   Copy android/key.properties.example and fill in the keystore values." >&2
    echo "   See docs/MOBILE-DISTRIBUTION-RUNBOOK.md §3 for the keystore ceremony." >&2
    exit 2
fi

echo "📦 Building TASMail Mobile ${BUILD_NAME}+${BUILD_NUMBER}"
echo "   (cwd: $MOBILE_DIR)"
echo

echo "→ flutter clean"
flutter clean

echo "→ flutter pub get"
flutter pub get

echo "→ flutter build appbundle --release"
flutter build appbundle --release \
    --build-name="$BUILD_NAME" \
    --build-number="$BUILD_NUMBER"

echo "→ flutter build apk --release (universal, for AppGallery + sideload)"
flutter build apk --release \
    --build-name="$BUILD_NAME" \
    --build-number="$BUILD_NUMBER"

AAB="build/app/outputs/bundle/release/app-release.aab"
APK="build/app/outputs/flutter-apk/app-release.apk"

echo
echo "✅ Build complete."
echo "   AAB: ${MOBILE_DIR}/${AAB} ($(du -h "$AAB" | cut -f1))"
echo "   APK: ${MOBILE_DIR}/${APK} ($(du -h "$APK" | cut -f1))"
echo
echo "Next steps:"
echo "   • Upload AAB to Play Console: cd android && bundle exec fastlane android internal"
echo "   • Upload APK to AppGallery:   cd android && bundle exec fastlane android appgallery"
