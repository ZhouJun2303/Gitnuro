#!/bin/sh
# Build the Mac app on a Mac. This machine's Git is the system Git.
# Do not copy the Windows MinGit tree into the app.
set -eu
cd "$(dirname "$0")/.."
pnpm install
pnpm tauri build
echo "The disk image is under src-tauri/target/release/bundle"
