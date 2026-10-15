#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../apps/desktop"
npm install
npm run tauri dev