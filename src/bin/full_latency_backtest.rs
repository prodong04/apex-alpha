use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

#[derive(Debug, Clone)]
struct TradeTick {
    ts: i64,
    price: f64,
}

#[derive(Debug, Clone)]
struct BacktestResult {
    delay_ms: i64,
    trades: usize,
    win_rate: f64,
    gross_pnl: f64,
    fees: f64,
    net_pnl: f64,
    ret_pct: f64,
    mdd: f64,
}

fn main() {
    println!("==================================================================");
    println!("🚀 APEX ALPHA: FULL DATASET (62.5 Hours) TAKER VS MAKER COMPARISON");
    println!("==================================================================");

    let start_load = Instant::now();
    let binance_path = "/Users/elias/Downloads/btc_market_data/binance_futures/btc_aggtrades_live.csv";
    let hl_path = "/Users/elias/Downloads/btc_market_data/hyperliquid/btc_trades_live.csv";

    let binance_ticks = load_all_binance_ticks(binance_path);
    let hl_ticks = load_all_hl_ticks(hl_path);

    println!("✅ Loaded {} Binance ticks & {} HL ticks in {:.2}s", 
        binance_ticks.len(), hl_ticks.len(), start_load.elapsed().as_secs_f64());

    let delays = vec![0, 50, 100, 200, 500];
    let spike_thresh = 100.0;
    let size_btc = 0.5;
    let initial_capital = 10000.0;

    println!("\n📊 1) MAKER (지정가 호가 제출, 수수료 0.01%):");
    println!("| Delay (ms) | Trades | Win Rate (%) | Gross Profit ($) | Fees ($) | Net PnL ($) | Return (%) | Max DD ($) |");
    println!("| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |");

    for &delay in &delays {
        let res = run_delayed_simulation(&binance_ticks, &hl_ticks, spike_thresh, delay, size_btc, 0.0001, initial_capital);
        println!(
            "| {:<10} | {:<6} | {:<12.1} | ${:<16.2} | ${:<8.2} | ${:<11.2} | {:<9.2}% | ${:<10.2} |",
            format!("+{} ms", delay), res.trades, res.win_rate, res.gross_pnl, res.fees, res.net_pnl, res.ret_pct, res.mdd
        );
    }

    println!("\n📊 2) TAKER (시장가 즉시 진입, 수수료 0.035%):");
    println!("| Delay (ms) | Trades | Win Rate (%) | Gross Profit ($) | Fees ($) | Net PnL ($) | Return (%) | Max DD ($) |");
    println!("| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |");

    for &delay in &delays {
        let res = run_delayed_simulation(&binance_ticks, &hl_ticks, spike_thresh, delay, size_btc, 0.00035, initial_capital);
        println!(
            "| {:<10} | {:<6} | {:<12.1} | ${:<16.2} | ${:<8.2} | ${:<11.2} | {:<9.2}% | ${:<10.2} |",
            format!("+{} ms", delay), res.trades, res.win_rate, res.gross_pnl, res.fees, res.net_pnl, res.ret_pct, res.mdd
        );
    }
    println!("==================================================================");
}

fn load_all_binance_ticks(path: &str) -> Vec<TradeTick> {
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

fn load_all_hl_ticks(path: &str) -> Vec<TradeTick> {
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

fn run_delayed_simulation(
    binance: &[TradeTick],
    hl: &[TradeTick],
    spike_thresh: f64,
    delay_ms: i64,
    size_btc: f64,
    fee_rate: f64,
    capital: f64,
) -> BacktestResult {
    let mut b_idx = 0;
    let mut hl_idx = 0;
    let b_len = binance.len();
    let hl_len = hl.len();

    let mut b_window: std::collections::VecDeque<(i64, f64)> = std::collections::VecDeque::new();
    let mut active_pos: Option<(i64, &'static str, f64, f64)> = None;
    let mut pending_order: Option<(i64, &'static str, f64)> = None;
    let mut cooldown_until = 0i64;

    let mut trades_net_pnls = Vec::new();
    let mut total_gross = 0.0;
    let mut total_fees = 0.0;

    let mut last_binance_px = binance[0].price;
    let mut last_hl_px = hl[0].price;

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

        while let Some(&(old_ts, _)) = b_window.front() {
            if cur_ts - old_ts > 1000 { b_window.pop_front(); } else { break; }
        }

        if let Some((target_exec_ts, side, delta)) = pending_order {
            if cur_ts >= target_exec_ts {
                let entry_px = last_hl_px;
                let target = if side == "LONG" { entry_px + (delta.abs() * 0.7) } else { entry_px - (delta.abs() * 0.7) };
                active_pos = Some((cur_ts, side, entry_px, target));
                pending_order = None;
            }
        }

        if let Some((entry_ts, side, entry_px, target_px)) = active_pos {
            let hold_time = cur_ts - entry_ts;
            let mut should_exit = false;

            if side == "LONG" {
                if last_hl_px >= target_px || last_hl_px <= entry_px - 40.0 || hold_time >= 3000 {
                    should_exit = true;
                }
            } else {
                if last_hl_px <= target_px || last_hl_px >= entry_px + 40.0 || hold_time >= 3000 {
                    should_exit = true;
                }
            }

            if should_exit {
                let exit_px = last_hl_px;
                let gross = if side == "LONG" { (exit_px - entry_px) * size_btc } else { (entry_px - exit_px) * size_btc };
                let fee = (entry_px + exit_px) * size_btc * fee_rate;
                let net = gross - fee;

                total_gross += gross;
                total_fees += fee;
                trades_net_pnls.push(net);

                active_pos = None;
                cooldown_until = cur_ts + 2000;
            }
        } else if pending_order.is_none() && cur_ts > cooldown_until && b_window.len() >= 2 {
            let oldest_px = b_window.front().unwrap().1;
            let delta = last_binance_px - oldest_px;

            if delta >= spike_thresh {
                pending_order = Some((cur_ts + delay_ms, "LONG", delta));
            } else if delta <= -spike_thresh {
                pending_order = Some((cur_ts + delay_ms, "SHORT", delta));
            }
        }
    }

    let trades = trades_net_pnls.len();
    if trades == 0 {
        return BacktestResult { delay_ms, trades: 0, win_rate: 0.0, gross_pnl: 0.0, fees: 0.0, net_pnl: 0.0, ret_pct: 0.0, mdd: 0.0 };
    }

    let wins = trades_net_pnls.iter().filter(|&&p| p > 0.0).count();
    let win_rate = (wins as f64 / trades as f64) * 100.0;
    let net_pnl: f64 = trades_net_pnls.iter().sum();
    let ret_pct = (net_pnl / capital) * 100.0;

    let mut peak = 0.0;
    let mut cum = 0.0;
    let mut mdd = 0.0;
    for &p in &trades_net_pnls {
        cum += p;
        if cum > peak { peak = cum; }
        let dd = peak - cum;
        if dd > mdd { mdd = dd; }
    }

    BacktestResult { delay_ms, trades, win_rate, gross_pnl: total_gross, fees: total_fees, net_pnl, ret_pct, mdd }
}
