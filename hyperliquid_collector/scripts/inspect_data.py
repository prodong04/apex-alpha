#!/usr/bin/env python3
"""
Multi-Exchange Live Market Data Inspector
=========================================
Inspect collected real-time market data across Binance Spot, Binance Futures, and Hyperliquid.
"""

import os
import csv
import argparse

def count_rows(file_path):
    if not os.path.exists(file_path):
        return 0
    with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
        return max(0, sum(1 for _ in f) - 1)

def get_last_row(file_path):
    if not os.path.exists(file_path):
        return None
    last = None
    with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
        reader = csv.DictReader(f)
        for row in reader:
            last = row
    return last

def inspect_all(coin="BTC", data_dir="data/live"):
    coin_lower = coin.lower()
    
    print("\n" + "=" * 75)
    print(f"📊 [UNIFIED MULTI-EXCHANGE DATA INSPECTOR] Symbol: {coin.upper()}")
    print("=" * 75)

    # 1. Binance Spot
    spot_dir = os.path.join(data_dir, "binance_spot")
    s_depth = os.path.join(spot_dir, f"{coin_lower}_depth20_live.csv")
    s_bbo = os.path.join(spot_dir, f"{coin_lower}_bbo_live.csv")
    s_trades = os.path.join(spot_dir, f"{coin_lower}_aggtrades_live.csv")

    print("\n🟡 [1. BINANCE SPOT (현물)]")
    print(f"  • L2 20-Depth (100ms) : {count_rows(s_depth):>8,} rows ({os.path.getsize(s_depth)/1024/1024:.2f} MB)" if os.path.exists(s_depth) else "  • L2 Depth : No data")
    print(f"  • Real-time BBO (0ms) : {count_rows(s_bbo):>8,} rows ({os.path.getsize(s_bbo)/1024/1024:.2f} MB)" if os.path.exists(s_bbo) else "  • BBO : No data")
    print(f"  • AggTrades (Ticks)   : {count_rows(s_trades):>8,} rows ({os.path.getsize(s_trades)/1024/1024:.2f} MB)" if os.path.exists(s_trades) else "  • AggTrades : No data")
    if last_trade := get_last_row(s_trades):
        print(f"    └ 최신 체결: ${float(last_trade.get('price', 0)):,.2f} | 수량: {last_trade.get('qty')} | 시간: {last_trade.get('trade_datetime_utc')}")

    # 2. Binance Futures
    fut_dir = os.path.join(data_dir, "binance_futures")
    f_depth = os.path.join(fut_dir, f"{coin_lower}_depth20_live.csv")
    f_bbo = os.path.join(fut_dir, f"{coin_lower}_bbo_live.csv")
    f_trades = os.path.join(fut_dir, f"{coin_lower}_aggtrades_live.csv")
    f_mark = os.path.join(fut_dir, f"{coin_lower}_mark_funding_live.csv")
    f_liqs = os.path.join(fut_dir, f"{coin_lower}_liquidations_live.csv")
    f_metrics = os.path.join(fut_dir, f"{coin_lower}_metrics_live.csv")

    print("\n🟢 [2. BINANCE FUTURES (선물)]")
    print(f"  • L2 20-Depth (100ms) : {count_rows(f_depth):>8,} rows" if os.path.exists(f_depth) else "  • L2 Depth : No data")
    print(f"  • Real-time BBO (0ms) : {count_rows(f_bbo):>8,} rows" if os.path.exists(f_bbo) else "  • BBO : No data")
    print(f"  • AggTrades (Ticks)   : {count_rows(f_trades):>8,} rows" if os.path.exists(f_trades) else "  • AggTrades : No data")
    print(f"  • Mark & Funding (1s) : {count_rows(f_mark):>8,} rows" if os.path.exists(f_mark) else "  • Mark/Funding : No data")
    print(f"  • Liquidations (청산) : {count_rows(f_liqs):>8,} rows" if os.path.exists(f_liqs) else "  • Liquidations : No data")
    print(f"  • REST Metrics (OI/LS): {count_rows(f_metrics):>8,} rows" if os.path.exists(f_metrics) else "  • Metrics : No data")
    if last_mark := get_last_row(f_mark):
        print(f"    └ Mark Price: ${float(last_mark.get('mark_px', 0)):,.2f} | Funding: {float(last_mark.get('funding_rate', 0))*100:.4f}%")
    if last_metrics := get_last_row(f_metrics):
        print(f"    └ Open Interest: {float(last_metrics.get('open_interest', 0)):,.1f} {coin} | 고래 롱숏비: {last_metrics.get('top_trader_long_short_pos_ratio')} | 개미 롱숏비: {last_metrics.get('global_long_short_acc_ratio')}")

    # 3. Hyperliquid
    hl_dir = os.path.join(data_dir, "hyperliquid")
    h_depth = os.path.join(hl_dir, f"{coin_lower}_depth20_live.csv")
    h_trades = os.path.join(hl_dir, f"{coin_lower}_trades_live.csv")
    h_metrics = os.path.join(hl_dir, f"{coin_lower}_metrics_live.csv")

    print("\n🔵 [3. HYPERLIQUID (DEX)]")
    print(f"  • L2 20-Depth (~5s)   : {count_rows(h_depth):>8,} rows" if os.path.exists(h_depth) else "  • L2 Depth : No data")
    print(f"  • Trades (Ticks)      : {count_rows(h_trades):>8,} rows" if os.path.exists(h_trades) else "  • Trades : No data")
    print(f"  • Metrics (1s OI/Fund): {count_rows(h_metrics):>8,} rows" if os.path.exists(h_metrics) else "  • Metrics : No data")
    if last_hl_metrics := get_last_row(h_metrics):
        print(f"    └ Oracle: ${float(last_hl_metrics.get('oracle_px', 0)):,.2f} | Funding: {float(last_hl_metrics.get('funding_rate', 0))*100:.6f}% | OI: {float(last_hl_metrics.get('open_interest', 0)):,.2f} {coin}")

    print("\n" + "=" * 75 + "\n")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Inspect Unified Multi-Exchange Live Market Data")
    parser.add_argument("--coin", default="BTC", help="Coin symbol (default: BTC)")
    parser.add_argument("--dir", default="data/live", help="Data directory (default: data/live)")
    args = parser.parse_args()

    inspect_all(args.coin, args.dir)

