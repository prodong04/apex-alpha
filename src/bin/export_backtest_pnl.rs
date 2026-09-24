use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};

#[derive(Debug, Clone)]
struct TradeTick {
    ts: i64,
    price: f64,
}

fn main() {
    let binance_path = "/Users/elias/Downloads/btc_market_data/binance_futures/btc_aggtrades_live.csv";
    let hl_path = "/Users/elias/Downloads/btc_market_data/hyperliquid/btc_trades_live.csv";

    let binance = load_binance(binance_path);
    let hl = load_hl(hl_path);

    let size_btc = 0.5;
    let maker_fee_rate = 0.0001; // 0.01%
    let taker_fee_rate = 0.00035; // 0.035%

    let mut b_idx = 0;
    let mut hl_idx = 0;
    let mut b_window: VecDeque<(i64, f64)> = VecDeque::new();
    let mut active_pos: Option<(i64, &'static str, f64, f64)> = None;
    let mut cooldown_until = 0i64;

    let mut last_binance_px = binance[0].price;
    let mut last_hl_px = hl[0].price;

    let mut cum_pnl_maker = 0.0;
    let mut cum_pnl_taker = 0.0;

    let out_path = "/Users/elias/apex-alpha/scratch/backtest_pnl_timeseries.csv";
    let _ = std::fs::create_dir_all("/Users/elias/apex-alpha/scratch");
    let mut out_file = File::create(out_path).unwrap();
    writeln!(out_file, "timestamp_ms,dt,side,entry_px,exit_px,holding_ms,gross_pnl,fee_maker,net_pnl_maker,cum_pnl_maker,fee_taker,net_pnl_taker,cum_pnl_taker,exit_reason").unwrap();

    while b_idx < binance.len() && hl_idx < hl.len() {
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

        while let Some(&(old_ts, _)) = b_window.front() {
            if cur_ts - old_ts > 1000 { b_window.pop_front(); } else { break; }
        }

        if let Some((entry_ts, side, entry_px, target_px)) = active_pos {
            let hold_time = cur_ts - entry_ts;
            let mut should_exit = false;
            let mut reason = "3s Timeout";

            if side == "LONG" {
                if last_hl_px >= target_px {
                    should_exit = true;
                    reason = "Target 70% Hit";
                } else if last_hl_px <= entry_px - 40.0 {
                    should_exit = true;
                    reason = "Stop Loss (-$40)";
                } else if hold_time >= 3000 {
                    should_exit = true;
                }
            } else {
                if last_hl_px <= target_px {
                    should_exit = true;
                    reason = "Target 70% Hit";
                } else if last_hl_px >= entry_px + 40.0 {
                    should_exit = true;
                    reason = "Stop Loss (-$40)";
                } else if hold_time >= 3000 {
                    should_exit = true;
                }
            }

            if should_exit {
                let exit_px = last_hl_px;
                let gross = if side == "LONG" { (exit_px - entry_px) * size_btc } else { (entry_px - exit_px) * size_btc };
                let fee_m = (entry_px + exit_px) * size_btc * maker_fee_rate;
                let net_m = gross - fee_m;
                cum_pnl_maker += net_m;

                let fee_t = (entry_px + exit_px) * size_btc * taker_fee_rate;
                let net_t = gross - fee_t;
                cum_pnl_taker += net_t;

                let dt = chrono::DateTime::from_timestamp_millis(cur_ts)
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_default();

                writeln!(
                    out_file,
                    "{},{},{},{:.2},{:.2},{},{:.2},{:.4},{:.2},{:.2},{:.4},{:.2},{:.2},{}",
                    cur_ts, dt, side, entry_px, exit_px, hold_time, gross, fee_m, net_m, cum_pnl_maker, fee_t, net_t, cum_pnl_taker, reason
                ).unwrap();

                active_pos = None;
                cooldown_until = cur_ts + 1500;
            }
        } else if cur_ts > cooldown_until && b_window.len() >= 2 {
            let oldest_px = b_window.front().unwrap().1;
            let delta = last_binance_px - oldest_px;

            if delta >= 80.0 {
                let limit_px = last_hl_px + 0.5;
                let target_px = limit_px + (delta * 0.7);
                active_pos = Some((cur_ts, "LONG", limit_px, target_px));
            } else if delta <= -80.0 {
                let limit_px = last_hl_px - 0.5;
                let target_px = limit_px - (delta.abs() * 0.7);
                active_pos = Some((cur_ts, "SHORT", limit_px, target_px));
            }
        }
    }

    println!("✅ Exported timeseries PnL CSV to {}", out_path);
}

fn load_binance(path: &str) -> Vec<TradeTick> {
    let file = File::open(path).unwrap();
    let reader = BufReader::with_capacity(32 * 1024 * 1024, file);
    let mut ticks = Vec::with_capacity(4_200_000);
    for line in reader.lines() {
        if let Ok(l) = line {
            if l.starts_with("agg_trade_id") || l.is_empty() { continue; }
            let mut parts = l.split(',');
            let _ = parts.next();
            if let Some(ts_str) = parts.next() {
                if let Ok(ts) = ts_str.parse::<i64>() {
                    let mut col_idx = 2;
                    let mut px = 0.0;
                    for p in parts {
                        if col_idx == 6 { px = p.parse().unwrap_or(0.0); break; }
                        col_idx += 1;
                    }
                    if px > 0.0 { ticks.push(TradeTick { ts, price: px }); }
                }
            }
        }
    }
    ticks
}

fn load_hl(path: &str) -> Vec<TradeTick> {
    let file = File::open(path).unwrap();
    let reader = BufReader::with_capacity(32 * 1024 * 1024, file);
    let mut ticks = Vec::with_capacity(1_000_000);
    for line in reader.lines() {
        if let Ok(l) = line {
            if l.starts_with("timestamp_ms") || l.is_empty() { continue; }
            let mut parts = l.split(',');
            if let Some(ts_str) = parts.next() {
                if let Ok(ts) = ts_str.parse::<i64>() {
                    let _dt = parts.next();
                    let _coin = parts.next();
                    let _side = parts.next();
                    let px = parts.next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
                    if px > 0.0 { ticks.push(TradeTick { ts, price: px }); }
                }
            }
        }
    }
    ticks
}
