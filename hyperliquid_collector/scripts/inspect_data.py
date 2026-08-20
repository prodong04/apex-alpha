#!/usr/bin/env python3
"""
Hyperliquid Live Market Data Inspector
======================================
Inspect collected real-time trades and L2 book depth files with summary statistics.
"""

import os
import csv
import sys
import argparse

def inspect_trades(coin="BTC", data_dir="data/live"):
    coin_lower = coin.lower()
    trades_file = os.path.join(data_dir, f"{coin_lower}_trades_live.csv")
    l2_file = os.path.join(data_dir, f"{coin_lower}_l2book_live.csv")

    if not os.path.exists(trades_file):
        print(f"[-] No trades file found at {trades_file}")
        return

    total_count = 0
    buy_vol = 0.0
    sell_vol = 0.0
    buy_count = 0
    sell_count = 0
    last_trades = []
    prices = []

    with open(trades_file, 'r', encoding='utf-8') as f:
        reader = csv.DictReader(f)
        for row in reader:
            total_count += 1
            sz = float(row['size'])
            px = float(row['price'])
            prices.append(px)
            
            if row['side'] == 'B':
                buy_vol += sz
                buy_count += 1
            else:
                sell_vol += sz
                sell_count += 1

            if len(last_trades) >= 5:
                last_trades.pop(0)
            last_trades.append(row)

    if total_count == 0:
        print(f"No trade records found in {trades_file}.")
        return

    total_vol = buy_vol + sell_vol
    min_px = min(prices)
    max_px = max(prices)
    last_px = prices[-1]

    print("\n" + "=" * 65)
    print(f"📊 [{coin.upper()} LIVE TRADES STATS] 총 수집 틱: {total_count:,}건")
    print("=" * 65)
    print(f"• 현재가: ${last_px:,.1f} | 최저가: ${min_px:,.1f} | 최고가: ${max_px:,.1f}")
    print(f"• 매수(Buy) 볼륨 : {buy_vol:12.4f} {coin.upper()} ({buy_count:,}건, {buy_vol/total_vol*100:.1f}%)")
    print(f"• 매도(Sell) 볼륨: {sell_vol:12.4f} {coin.upper()} ({sell_count:,}건, {sell_vol/total_vol*100:.1f}%)")
    print(f"• 총 거래량       : {total_vol:12.4f} {coin.upper()}")
    print("-" * 65)
    print("🕒 [최근 5건의 실시간 체결 틱]")
    print(f"{'시간 (UTC)':<23} | {'사이드':<4} | {'가격 ($)':<11} | {'수량':<12}")
    print("-" * 65)
    for t in last_trades:
        side_label = "BUY " if t['side'] == 'B' else "SELL"
        print(f"{t['datetime_utc']:<23} | {side_label:<4} | {float(t['price']):<11.1f} | {float(t['size']):<12.5f}")
    print("=" * 65 + "\n")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Inspect Hyperliquid live market data")
    parser.add_argument("--coin", default="BTC", help="Coin symbol (default: BTC)")
    parser.add_argument("--dir", default="data/live", help="Data directory (default: data/live)")
    args = parser.parse_args()

    inspect_trades(args.coin, args.dir)
