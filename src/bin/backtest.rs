use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

#[derive(Debug, Clone)]
struct TradeTick {
    ts: i64,
    price: f64,
    qty: f64,
    is_buy: bool,
}

#[derive(Debug, Clone)]
struct BacktestTrade {
    side: &'static str,
    entry_px: f64,
    exit_px: f64,
    gross_pnl: f64,
    fee: f64,
    net_pnl: f64,
    holding_ms: i64,
    reason: &'static str,
}

fn main() {
    println!("==================================================================");
    println!("🚀 APEX ALPHA: 3-Hour Real Momentum Spike Lead-Lag Backtest");
    println!("==================================================================");

    let start_load = Instant::now();
    let binance_path = "/Users/elias/Downloads/btc_market_data/binance_futures/btc_aggtrades_live.csv";
    let hl_path = "/Users/elias/Downloads/btc_market_data/hyperliquid/btc_trades_live.csv";

    let window_ms = 3 * 3600 * 1000i64;
    let end_ts = 1787555582000i64;
    let start_ts = end_ts - window_ms;

    let binance_ticks = load_binance_ticks(binance_path, start_ts, end_ts);
    let hl_ticks = load_hl_ticks(hl_path, start_ts, end_ts);

    println!("✅ Loaded {} Binance ticks & {} HL ticks in {:.2}s", 
        binance_ticks.len(), hl_ticks.len(), start_load.elapsed().as_secs_f64());

    let spikes = vec![40.0, 60.0, 80.0, 100.0, 120.0]; // Binance 1-sec Delta Spike ($)
    let lookback_ms = 1000i64; // 1 second lookback

    println!("\n🔍 Testing Real Binance 1-Second Price Spikes (Delta Shock) vs Hyperliquid Reaction:\n");
    println!("| Spike Shock ($/1s) | Trades | Win Rate (%) | Gross PnL ($) | Total Fees ($) | Net PnL ($) | Return (%) | Max DD ($) | Profit Factor |");
    println!("| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |");

    let initial_capital = 10000.0;
    let size_btc = 0.5;
    let taker_fee_rate = 0.00035;

    let mut best_pnl = -f64::INFINITY;
    let mut best_trades = Vec::new();
    let mut best_spike = 0.0;

    for &spike in &spikes {
        let trades = run_spike_simulation(
            &binance_ticks,
            &hl_ticks,
            spike,
            lookback_ms,
            size_btc,
            taker_fee_rate,
        );

        if trades.is_empty() {
            continue;
        }

        let total = trades.len();
        let wins = trades.iter().filter(|t| t.net_pnl > 0.0).count();
        let win_rate = (wins as f64 / total as f64) * 100.0;
        let gross: f64 = trades.iter().map(|t| t.gross_pnl).sum();
        let fees: f64 = trades.iter().map(|t| t.fee).sum();
        let net: f64 = trades.iter().map(|t| t.net_pnl).sum();
        let ret = (net / initial_capital) * 100.0;

        let gross_wins: f64 = trades.iter().filter(|t| t.gross_pnl > 0.0).map(|t| t.gross_pnl).sum();
        let gross_losses: f64 = trades.iter().filter(|t| t.gross_pnl < 0.0).map(|t| -t.gross_pnl).sum();
        let pf = if gross_losses > 0.0 { gross_wins / gross_losses } else { gross_wins };

        let mut peak = 0.0;
        let mut cum = 0.0;
        let mut mdd = 0.0;
        for t in &trades {
            cum += t.net_pnl;
            if cum > peak { peak = cum; }
            let dd = peak - cum;
            if dd > mdd { mdd = dd; }
        }

        println!(
            "| ${:<18.1} | {:<6} | {:<12.1} | ${:<12.2} | ${:<13.2} | ${:<10.2} | {:<9.2}% | ${:<9.2} | {:<13.2} |",
            spike, total, win_rate, gross, fees, net, ret, mdd, pf
        );

        if net > best_pnl {
            best_pnl = net;
            best_trades = trades;
            best_spike = spike;
        }
    }

    println!("\n==================================================================");
    println!("🏆 OPTIMAL LEAD-LAG SPIKE RESULT");
    println!("==================================================================");
    println!("• Spike Threshold : ${:.1} / 1s", best_spike);
    println!("• Total Trades    : {}", best_trades.len());
    let wins = best_trades.iter().filter(|t| t.net_pnl > 0.0).count();
    println!("• Win Rate        : {:.2}% ({}/{} wins)", (wins as f64 / best_trades.len() as f64) * 100.0, wins, best_trades.len());
    let gross: f64 = best_trades.iter().map(|t| t.gross_pnl).sum();
    let fees: f64 = best_trades.iter().map(|t| t.fee).sum();
    println!("• Gross Profit    : +${:.2}", gross);
    println!("• Total Fees Paid : -${:.2}", fees);
    println!("• Net Profit      : +${:.2} (+{:.2}%)", best_pnl, (best_pnl / initial_capital) * 100.0);

    println!("\n📋 Sample Execution Trades:");
    println!("------------------------------------------------------------------");
    println!("Side | Entry Px ($) | Exit Px ($) | Hold (ms) | Gross ($) | Net PnL ($) | Reason");
    println!("------------------------------------------------------------------");
    for t in best_trades.iter().take(10) {
        println!(
            "{:<4} | ${:<11.2} | ${:<10.2} | {:<9} | ${:<8.2} | ${:<10.2} | {}",
            t.side, t.entry_px, t.exit_px, t.holding_ms, t.gross_pnl, t.net_pnl, t.reason
        );
    }
    println!("==================================================================");
}

fn load_binance_ticks(path: &str, start_ts: i64, end_ts: i64) -> Vec<TradeTick> {
    let file = File::open(path).unwrap();
    let reader = BufReader::with_capacity(16 * 1024 * 1024, file);
    let mut ticks = Vec::with_capacity(300_000);

    for line in reader.lines() {
        if let Ok(l) = line {
            if l.starts_with("agg_trade_id") || l.is_empty() { continue; }
            let mut parts = l.split(',');
            let _ = parts.next();
            if let Some(ts_str) = parts.next() {
                if let Ok(ts) = ts_str.parse::<i64>() {
                    if ts < start_ts { continue; }
                    if ts > end_ts { break; }
                    let mut col_idx = 2;
                    let mut px = 0.0;
                    let mut qty = 0.0;
                    let mut is_buyer_maker = false;
                    for p in parts {
                        if col_idx == 6 { px = p.parse().unwrap_or(0.0); }
                        else if col_idx == 7 { qty = p.parse().unwrap_or(0.0); }
                        else if col_idx == 10 { is_buyer_maker = p == "true"; }
                        col_idx += 1;
                    }
                    if px > 0.0 {
                        ticks.push(TradeTick { ts, price: px, qty, is_buy: !is_buyer_maker });
                    }
                }
            }
        }
    }
    ticks
}

fn load_hl_ticks(path: &str, start_ts: i64, end_ts: i64) -> Vec<TradeTick> {
    let file = File::open(path).unwrap();
    let reader = BufReader::with_capacity(16 * 1024 * 1024, file);
    let mut ticks = Vec::with_capacity(100_000);

    for line in reader.lines() {
        if let Ok(l) = line {
            if l.starts_with("timestamp_ms") || l.is_empty() { continue; }
            let mut parts = l.split(',');
            if let Some(ts_str) = parts.next() {
                if let Ok(ts) = ts_str.parse::<i64>() {
                    if ts < start_ts { continue; }
                    if ts > end_ts { break; }
                    let _dt = parts.next();
                    let _coin = parts.next();
                    let side = parts.next().unwrap_or("B");
                    let px = parts.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
                    let size = parts.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);

                    if px > 0.0 {
                        ticks.push(TradeTick { ts, price: px, qty: size, is_buy: side == "B" });
                    }
                }
            }
        }
    }
    ticks
}

fn run_spike_simulation(
    binance: &[TradeTick],
    hl: &[TradeTick],
    spike_thresh: f64,
    lookback_ms: i64,
    size_btc: f64,
    fee_rate: f64,
) -> Vec<BacktestTrade> {
    let mut trades = Vec::new();

    let mut b_idx = 0;
    let mut hl_idx = 0;
    let b_len = binance.len();
    let hl_len = hl.len();

    let mut active_pos: Option<(i64, &'static str, f64, f64)> = None; // (entry_ts, side, entry_px, target_px)
    let mut last_binance_px = binance[0].price;
    let mut last_hl_px = hl[0].price;

    let mut b_window: std::collections::VecDeque<(i64, f64)> = std::collections::VecDeque::new();
    let mut cooldown_until = 0i64;

    while b_idx < b_len && hl_idx < hl_len {
        let b_t = &binance[b_idx];
        let hl_t = &hl[hl_idx];

        let cur_ts;
        if b_t.ts <= hl_t.ts {
            cur_ts = b_t.ts;
            last_binance_px = b_t.price;
            b_window.push_back((b_t.ts, b_t.price));
            b_idx += 1;
        } else {
            cur_ts = hl_t.ts;
            last_hl_px = hl_t.price;
            hl_idx += 1;
        }

        // Clean old window
        while let Some(&(old_ts, _)) = b_window.front() {
            if cur_ts - old_ts > lookback_ms {
                b_window.pop_front();
            } else {
                break;
            }
        }

        // 1. Position management
        if let Some((entry_ts, side, entry_px, target_px)) = active_pos {
            let hold_time = cur_ts - entry_ts;
            let mut should_exit = false;
            let mut reason = "Timeout (3s)";

            if side == "LONG" {
                if last_hl_px >= target_px {
                    should_exit = true;
                    reason = "Target Catch-up (+Spike)";
                } else if last_hl_px <= entry_px - 40.0 {
                    should_exit = true;
                    reason = "Stop Loss (-$40)";
                } else if hold_time >= 3000 {
                    should_exit = true;
                    reason = "Timeout (3s)";
                }
            } else {
                if last_hl_px <= target_px {
                    should_exit = true;
                    reason = "Target Catch-up (-Spike)";
                } else if last_hl_px >= entry_px + 40.0 {
                    should_exit = true;
                    reason = "Stop Loss (-$40)";
                } else if hold_time >= 3000 {
                    should_exit = true;
                    reason = "Timeout (3s)";
                }
            }

            if should_exit {
                let exit_px = last_hl_px;
                let gross_pnl = if side == "LONG" {
                    (exit_px - entry_px) * size_btc
                } else {
                    (entry_px - exit_px) * size_btc
                };
                let fee = (entry_px + exit_px) * size_btc * fee_rate;
                let net_pnl = gross_pnl - fee;

                trades.push(BacktestTrade {
                    side,
                    entry_px,
                    exit_px,
                    gross_pnl,
                    fee,
                    net_pnl,
                    holding_ms: hold_time,
                    reason,
                });

                active_pos = None;
                cooldown_until = cur_ts + 2000; // 2s cooldown
            }
        } else if cur_ts > cooldown_until && b_window.len() >= 2 {
            // 2. Shock Detection: Calculate 1-second price delta on Binance
            let oldest_px = b_window.front().unwrap().1;
            let delta = last_binance_px - oldest_px;

            if delta >= spike_thresh {
                // Binance surged by spike_thresh in 1 second!
                // Sniping Long on Hyperliquid targeting 70% of the surge
                let target = last_hl_px + (delta * 0.7);
                active_pos = Some((cur_ts, "LONG", last_hl_px, target));
            } else if delta <= -spike_thresh {
                // Binance dumped by spike_thresh in 1 second!
                // Sniping Short on Hyperliquid
                let target = last_hl_px + (delta * 0.7);
                active_pos = Some((cur_ts, "SHORT", last_hl_px, target));
            }
        }
    }

    trades
}
