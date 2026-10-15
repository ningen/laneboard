#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/apps/desktop"
npm install
npm run build
rm -rf "$ROOT/crates/cli/embedded-ui"
mkdir -p "$ROOT/crates/cli/embedded-ui"
cp -R dist/. "$ROOT/crates/cli/embedded-ui/"
# placeholder so the folder is never empty for rust-embed
test -f "$ROOT/crates/cli/embedded-ui/index.html"
echo "Embedded UI → crates/cli/embedded-ui/"