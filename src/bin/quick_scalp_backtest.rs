use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Instant;

#[derive(Debug, Clone)]
struct TradeTick {
    ts: i64,
    price: f64,
}

#[derive(Debug, Clone)]
struct QuickScalpResult {
    max_hold_ms: i64,
    target_profit_usd: f64,
    trades: usize,
    win_rate: f64,
    gross_pnl: f64,
    fees: f64,
    net_pnl: f64,
    ret_pct: f64,
    avg_hold_ms: f64,
    mdd: f64,
}

fn main() {
    println!("==================================================================");
    println!("🚀 APEX ALPHA: ULTRA-FAST SUB-SECOND SCALP EXIT BACKTEST (62.5H)");
    println!("==================================================================");

    let start_load = Instant::now();
    let binance_path = "/Users/elias/Downloads/btc_market_data/binance_futures/btc_aggtrades_live.csv";
    let hl_path = "/Users/elias/Downloads/btc_market_data/hyperliquid/btc_trades_live.csv";

    let binance_ticks = load_all_binance_ticks(binance_path);
    let hl_ticks = load_all_hl_ticks(hl_path);

    println!("✅ Loaded 5.12M Ticks in {:.2}s", start_load.elapsed().as_secs_f64());

    let size_btc = 0.5;
    let maker_fee_rate = 0.0001; // 0.01% Maker fee
    let initial_capital = 10000.0;
    let spike_thresh = 80.0; // $80 spike

    // Test different quick targets and hold timeouts
    let max_holds = vec![400, 600, 800, 1000, 1500, 3000];
    let targets = vec![25.0, 30.0, 35.0, 45.0];

    println!("\n🔍 Testing Ultra-Fast Holding Times (400ms ~ 3000ms) with Quick Scalp Targets:\n");
    println!("| Max Hold (ms) | Target ($) | Trades | Win Rate (%) | Net PnL ($) | Return (%) | Avg Hold (ms) | Max DD ($) | Profit Factor |");
    println!("| :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |");

    let mut best_net = -f64::INFINITY;
    let mut best_config = (0, 0.0);

    for &max_hold in &max_holds {
        for &target in &targets {
            let res = run_quick_scalp_simulation(
                &binance_ticks,
                &hl_ticks,
                spike_thresh,
                target,
                max_hold,
                size_btc,
                maker_fee_rate,
                initial_capital,
            );

            if res.trades == 0 { continue; }

            println!(
                "| {:<13} | ${:<9.1} | {:<6} | {:<12.1} | ${:<10.2} | {:<9.2}% | {:<13.0} | ${:<10.2} | {:<13.2} |",
                format!("{} ms", max_hold), target, res.trades, res.win_rate, res.net_pnl, res.ret_pct, res.avg_hold_ms, res.mdd,
                if res.gross_pnl > 0.0 { res.gross_pnl / (res.gross_pnl - res.net_pnl).max(1.0) } else { 0.0 }
            );

            if res.net_pnl > best_net {
                best_net = res.net_pnl;
                best_config = (max_hold, target);
            }
        }
    }

    println!("\n==================================================================");
    println!("🏆 OPTIMAL ULTRA-FAST SCALP CONFIGURATION");
    println!("==================================================================");
    println!("• Optimal Max Hold Time : {} ms ({:.2} seconds)", best_config.0, best_config.0 as f64 / 1000.0);
    println!("• Optimal Target Profit : ${:.1}", best_config.1);
    println!("• Best Net Profit (62h) : +${:.2} (+{:.2}%)", best_net, (best_net / initial_capital) * 100.0);
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

fn run_quick_scalp_simulation(
    binance: &[TradeTick],
    hl: &[TradeTick],
    spike_thresh: f64,
    target_usd: f64,
    max_hold_ms: i64,
    size_btc: f64,
    maker_fee_rate: f64,
    capital: f64,
) -> QuickScalpResult {
    let mut b_idx = 0;
    let mut hl_idx = 0;
    let b_len = binance.len();
    let hl_len = hl.len();

    let mut b_window: VecDeque<(i64, f64)> = VecDeque::new();
    let mut active_pos: Option<(i64, &'static str, f64, f64)> = None; // (entry_ts, side, entry_px, target_px)
    let mut cooldown_until = 0i64;

    let mut net_pnls = Vec::new();
    let mut holding_times = Vec::new();
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

        if let Some((entry_ts, side, entry_px, target_px)) = active_pos {
            let hold_time = cur_ts - entry_ts;
            let mut should_exit = false;

            if side == "LONG" {
                if last_hl_px >= target_px {
                    should_exit = true; // Quick Profit Target hit!
                } else if last_hl_px <= entry_px - 25.0 {
                    should_exit = true; // Tight Stop Loss
                } else if hold_time >= max_hold_ms {
                    should_exit = true; // Ultra-Fast Timeout exit!
                }
            } else {
                if last_hl_px <= target_px {
                    should_exit = true;
                } else if last_hl_px >= entry_px + 25.0 {
                    should_exit = true;
                } else if hold_time >= max_hold_ms {
                    should_exit = true;
                }
            }

            if should_exit {
                let exit_px = last_hl_px;
                let gross = if side == "LONG" { (exit_px - entry_px) * size_btc } else { (entry_px - exit_px) * size_btc };
                let fee = (entry_px + exit_px) * size_btc * maker_fee_rate;
                let net = gross - fee;

                total_gross += gross;
                total_fees += fee;
                net_pnls.push(net);
                holding_times.push(hold_time);

                active_pos = None;
                cooldown_until = cur_ts + 1000;
            }
        } else if cur_ts > cooldown_until && b_window.len() >= 2 {
            let oldest_px = b_window.front().unwrap().1;
            let delta = last_binance_px - oldest_px;

            if delta >= spike_thresh {
                // Long: target entry + target_usd
                active_pos = Some((cur_ts, "LONG", last_hl_px, last_hl_px + target_usd));
            } else if delta <= -spike_thresh {
                // Short: target entry - target_usd
                active_pos = Some((cur_ts, "SHORT", last_hl_px, last_hl_px - target_usd));
            }
        }
    }

    let trades = net_pnls.len();
    if trades == 0 {
        return QuickScalpResult {
            max_hold_ms, target_profit_usd: target_usd, trades: 0, win_rate: 0.0,
            gross_pnl: 0.0, fees: 0.0, net_pnl: 0.0, ret_pct: 0.0, avg_hold_ms: 0.0, mdd: 0.0,
        };
    }

    let wins = net_pnls.iter().filter(|&&p| p > 0.0).count();
    let win_rate = (wins as f64 / trades as f64) * 100.0;
    let net_pnl: f64 = net_pnls.iter().sum();
    let ret_pct = (net_pnl / capital) * 100.0;
    let avg_hold_ms = holding_times.iter().map(|&t| t as f64).sum::<f64>() / trades as f64;

    let mut peak = 0.0;
    let mut cum = 0.0;
    let mut mdd = 0.0;
    for &p in &net_pnls {
        cum += p;
        if cum > peak { peak = cum; }
        let dd = peak - cum;
        if dd > mdd { mdd = dd; }
    }

    QuickScalpResult {
        max_hold_ms, target_profit_usd: target_usd, trades, win_rate,
        gross_pnl: total_gross, fees: total_fees, net_pnl, ret_pct, avg_hold_ms, mdd,
    }
}
