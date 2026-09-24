#!/usr/bin/env bash
set -e

echo "=================================================================="
echo "🚀 APEX ALPHA: 5-YEAR INSTITUTIONAL STAT-ARB BACKTEST"
echo "=================================================================="

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$DIR/engine"

echo "• Compiling Optimized Release Binary..."
cargo build --release --bin run_backtest_5year

echo "• Running 5-Year Point-In-Time Continuous Simulation (2022-2026)..."
cargo run --release --bin run_backtest_5year

echo "• Updating Executive Dashboard & Vector SVG..."
python3 "$DIR/scripts/generate_report.py"

echo "✅ Backtest & Dashboard Regeneration Complete!"
