#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
rustup target add x86_64-pc-windows-gnu
cargo build -p board --release --target x86_64-pc-windows-gnu
mkdir -p dist
cp -f target/x86_64-pc-windows-gnu/release/board.exe dist/board-windows-x64.exe
file dist/board-windows-x64.exe
ls -lh dist/board-windows-x64.exe
echo "Wrote dist/board-windows-x64.exe"