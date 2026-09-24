#!/usr/bin/env bash
set -e

echo "=================================================================="
echo "🚀 Launching APEX ALPHA Unified Rust Trading Engine & Dashboard"
echo "=================================================================="

# Check if release flag is provided
if [ "$1" == "--release" ] || [ "$1" == "-r" ]; then
    echo "⚡ Running in RELEASE Mode (Optimized)..."
    cargo run --release
else
    echo "🛠️ Running in DEV Mode..."
    cargo run
fi
