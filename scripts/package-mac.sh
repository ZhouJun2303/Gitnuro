#!/bin/sh
# Build the Mac app on a Mac. This machine's Git is the system Git.
# Do not copy the Windows MinGit tree into the app.
# Signing and notarization need Apple certificates. This script does not
# invent a signature. Set AWE_REQUIRE_SIGN=1 only when APPLE_SIGNING_IDENTITY
# is a real Developer ID, then pass it to the Tauri bundler yourself.
set -eu
cd "$(dirname "$0")/.."
if [ "${AWE_REQUIRE_SIGN:-}" = "1" ] && [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  echo "AWE_REQUIRE_SIGN=1 but APPLE_SIGNING_IDENTITY is missing." >&2
  echo "Create a Developer ID Application certificate, export it, and set APPLE_SIGNING_IDENTITY." >&2
  echo "Notarization also needs an Apple ID and an app-specific password. This script will not emit an unsigned package while signing is required." >&2
  exit 1
fi
if [ -z "${APPLE_SIGNING_IDENTITY:-}" ]; then
  echo "APPLE_SIGNING_IDENTITY is not set. The bundle will be unsigned and not notarized."
fi
pnpm install
pnpm tauri build
echo "The disk image is under target/release/bundle"
