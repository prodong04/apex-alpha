use apex_engine::dtw::clustering::{cluster_into_baskets, compute_dtw_distance_matrix, ClusterBasket};
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

struct KlineBar {
    ts: i64,
    close: f64,
}

struct TradeRecord {
    entry_idx: usize,
    entry_ts: i64,
    exit_ts: i64,
    cluster_id: usize,
    leader: String,
    leader_ret: f64,
    laggards: Vec<String>,
    holding_bars: usize,
    gross_return: f64,
    net_pnl_usd: f64,
    exit_reason: String,
}

fn load_klines(data_dir: &str) -> (Vec<String>, Vec<i64>, HashMap<String, Vec<f64>>) {
    let mut raw_data: HashMap<String, BTreeMap<i64, f64>> = HashMap::new();
    let mut all_timestamps = std::collections::BTreeSet::new();

    let entries = std::fs::read_dir(data_dir).unwrap();
    for entry in entries {
        if let Ok(e) = entry {
            let path = e.path();
            if path.extension().and_then(|s| s.to_str()) == Some("csv") {
                let filename = path.file_stem().unwrap().to_str().unwrap();
                let symbol = filename.replace("_1m", "");

                let file = File::open(&path).unwrap();
                let reader = BufReader::new(file);
                let mut map = BTreeMap::new();

                for (idx, line) in reader.lines().enumerate() {
                    if idx == 0 { continue; }
                    if let Ok(l) = line {
                        let parts: Vec<&str> = l.split(',').collect();
                        if parts.len() >= 5 {
                            let ts: i64 = parts[0].parse().unwrap_or(0);
                            let close: f64 = parts[5].parse().unwrap_or(0.0);
                            if ts > 0 && close > 0.0 {
                                map.insert(ts, close);
                                all_timestamps.insert(ts);
                            }
                        }
                    }
                }

                if map.len() > 1000 {
                    raw_data.insert(symbol, map);
                }
            }
        }
    }

    // Align all symbols by intersection of timestamps
    let mut valid_timestamps: Vec<i64> = all_timestamps.into_iter().collect();
    let symbols: Vec<String> = raw_data.keys().cloned().collect();

    // Filter timestamps where ALL symbols have prices
    valid_timestamps.retain(|ts| symbols.iter().all(|s| raw_data[s].contains_key(ts)));

    let mut price_series: HashMap<String, Vec<f64>> = HashMap::new();
    for s in &symbols {
        let mut prices = Vec::with_capacity(valid_timestamps.len());
        for ts in &valid_timestamps {
            prices.push(*raw_data[s].get(ts).unwrap());
        }
        price_series.insert(s.clone(), prices);
    }

    (symbols, valid_timestamps, price_series)
}

fn main() {
    let data_dir = "/Users/elias/apex-alpha/data/klines";
    println!("📂 Loading 1-minute historical datasets from {}...", data_dir);
    let (symbols, timestamps, price_series) = load_klines(data_dir);

    let total_bars = timestamps.len();
    println!("✅ Synchronized {} symbols across {} continuous 1m bars (~{:.1} days)",
             symbols.len(), total_bars, (total_bars as f64) / 1440.0);
    println!("• Universe: {:?}", symbols);

    let lookback_bars = 60; // 60 mins for DTW clustering
    let recluster_interval = 30; // Re-cluster every 30 mins
    let num_clusters = 4; // 4 dynamic baskets
    let dtw_window_radius = 8; // Max 8-min warping

    let leader_momentum_threshold = 0.008; // Leader +0.8% in 3 bars
    let laggard_max_return = 0.0025; // Laggard <= +0.25% (unmoved)
    let target_take_profit = 0.007; // +0.7% basket profit
    let stop_loss = -0.005; // -0.5% basket loss
    let max_holding_bars = 15; // 15 mins timeout
    let fee_rate = 0.0006; // 0.06% roundtrip taker fee
    let basket_capital_usd = 1000.0; // $1,000 per basket trade

    let mut current_baskets: Vec<ClusterBasket> = Vec::new();
    let mut trades: Vec<TradeRecord> = Vec::new();
    let mut last_entry_bar: HashMap<usize, usize> = HashMap::new(); // cluster_id -> last_bar

    println!("\n🚀 Running Rolling DTW Momentum Spillover Simulation...");

    for bar_idx in lookback_bars..(total_bars - max_holding_bars) {
        // 1. Periodically Re-Cluster using DTW
        if (bar_idx - lookback_bars) % recluster_interval == 0 {
            let mut sub_series = Vec::new();
            for s in &symbols {
                let p = &price_series[s][(bar_idx - lookback_bars)..bar_idx];
                sub_series.push(p.to_vec());
            }

            let dist_matrix = compute_dtw_distance_matrix(&symbols, &sub_series, dtw_window_radius);
            current_baskets = cluster_into_baskets(&symbols, &dist_matrix, num_clusters);
        }

        // 2. Scan each basket for Momentum Spillover Opportunity
        for basket in &current_baskets {
            if basket.symbols.len() < 2 { continue; }

            // Cool-down check per cluster
            if let Some(&last_bar) = last_entry_bar.get(&basket.cluster_id) {
                if bar_idx < last_bar + 10 { continue; }
            }

            // Check 3-bar momentum for each symbol in basket
            let mut leader_symbol = None;
            let mut leader_ret = 0.0;

            for s in &basket.symbols {
                let p_curr = price_series[s][bar_idx];
                let p_prev = price_series[s][bar_idx - 3];
                let ret = (p_curr - p_prev) / p_prev;

                if ret >= leader_momentum_threshold && ret > leader_ret {
                    leader_ret = ret;
                    leader_symbol = Some(s.clone());
                }
            }

            if let Some(leader) = leader_symbol {
                // Find laggards in the same DTW cluster
                let mut laggards = Vec::new();
                for s in &basket.symbols {
                    if s == &leader { continue; }
                    let p_curr = price_series[s][bar_idx];
                    let p_prev = price_series[s][bar_idx - 3];
                    let ret = (p_curr - p_prev) / p_prev;

                    if ret <= laggard_max_return {
                        laggards.push(s.clone());
                    }
                }

                if !laggards.is_empty() {
                    // Enter Basket Trade!
                    let entry_ts = timestamps[bar_idx];
                    let mut entry_prices: HashMap<String, f64> = HashMap::new();
                    for lag in &laggards {
                        entry_prices.insert(lag.clone(), price_series[lag][bar_idx]);
                    }

                    // Simulate trade lifecycle forward
                    let mut exit_bar = bar_idx + max_holding_bars;
                    let mut exit_reason = "TIMEOUT_15M".to_string();
                    let mut exit_ret = 0.0;

                    for forward_bar in (bar_idx + 1)..=(bar_idx + max_holding_bars) {
                        let mut basket_current_ret = 0.0;
                        for lag in &laggards {
                            let p_fwd = price_series[lag][forward_bar];
                            let p_ent = entry_prices[lag];
                            basket_current_ret += (p_fwd - p_ent) / p_ent;
                        }
                        basket_current_ret /= laggards.len() as f64;

                        if basket_current_ret >= target_take_profit {
                            exit_bar = forward_bar;
                            exit_reason = "TAKE_PROFIT".to_string();
                            exit_ret = basket_current_ret;
                            break;
                        } else if basket_current_ret <= stop_loss {
                            exit_bar = forward_bar;
                            exit_reason = "STOP_LOSS".to_string();
                            exit_ret = basket_current_ret;
                            break;
                        }

                        if forward_bar == bar_idx + max_holding_bars {
                            exit_ret = basket_current_ret;
                        }
                    }

                    let holding_bars = exit_bar - bar_idx;
                    let net_return = exit_ret - fee_rate;
                    let net_pnl_usd = basket_capital_usd * net_return;

                    trades.push(TradeRecord {
                        entry_idx: bar_idx,
                        entry_ts,
                        exit_ts: timestamps[exit_bar],
                        cluster_id: basket.cluster_id,
                        leader,
                        leader_ret,
                        laggards,
                        holding_bars,
                        gross_return: exit_ret,
                        net_pnl_usd,
                        exit_reason,
                    });

                    last_entry_bar.insert(basket.cluster_id, exit_bar);
                }
            }
        }
    }

    print_results(&trades, symbols.len(), total_bars);
}

fn print_results(trades: &[TradeRecord], num_symbols: usize, total_bars: usize) {
    println!("\n==================================================================");
    println!("📊 DTW CLUSTERING MOMENTUM SPILLOVER BACKTEST REPORT");
    println!("==================================================================");
    println!("• Dataset Range       : {:.1} Days ({} 1-min bars across {} tokens)",
             (total_bars as f64)/1440.0, total_bars, num_symbols);
    println!("• Total Basket Trades : {} trades", trades.len());

    if trades.is_empty() {
        println!("No trades generated.");
        return;
    }

    let wins = trades.iter().filter(|t| t.net_pnl_usd > 0.0).count();
    let losses = trades.len() - wins;
    let win_rate = (wins as f64 / trades.len() as f64) * 100.0;

    let total_net_pnl: f64 = trades.iter().map(|t| t.net_pnl_usd).sum();
    let total_gross_profit: f64 = trades.iter().filter(|t| t.net_pnl_usd > 0.0).map(|t| t.net_pnl_usd).sum();
    let total_gross_loss: f64 = trades.iter().filter(|t| t.net_pnl_usd <= 0.0).map(|t| t.net_pnl_usd.abs()).sum();
    let profit_factor = if total_gross_loss > 0.0 { total_gross_profit / total_gross_loss } else { 99.9 };

    let avg_holding: f64 = trades.iter().map(|t| t.holding_bars as f64).sum::<f64>() / trades.len() as f64;

    let tp_count = trades.iter().filter(|t| t.exit_reason == "TAKE_PROFIT").count();
    let sl_count = trades.iter().filter(|t| t.exit_reason == "STOP_LOSS").count();
    let to_count = trades.iter().filter(|t| t.exit_reason == "TIMEOUT_15M").count();

    println!("------------------------------------------------------------------");
    println!("🏆 PERFORMANCE METRICS:");
    println!("  - Win Rate (승률)       : {:.1}% ({} Wins / {} Losses)", win_rate, wins, losses);
    println!("  - Total Net PnL (순익)  : ${:.2} (기본 바스켓당 $1,000 기준)", total_net_pnl);
    println!("  - Profit Factor (손익비): {:.2}", profit_factor);
    println!("  - Avg Holding Time      : {:.1} mins", avg_holding);
    println!("------------------------------------------------------------------");
    println!("🎯 EXIT BREAKDOWN:");
    println!("  - 🟢 Take Profit (+0.7%) : {:>3} trades ({:.1}%)", tp_count, (tp_count as f64 / trades.len() as f64)*100.0);
    println!("  - 🔴 Stop Loss (-0.5%)   : {:>3} trades ({:.1}%)", sl_count, (sl_count as f64 / trades.len() as f64)*100.0);
    println!("  - ⏱️ 15m Timeout Exit    : {:>3} trades ({:.1}%)", to_count, (to_count as f64 / trades.len() as f64)*100.0);
    println!("==================================================================");

    println!("\n🔍 SAMPLE SPILLOVER BASKET TRADES (최근 10건 샘플):");
    println!("| 진입 일시 (UTC) | 리더 코인 (3분 상승) | 래거 매수 바스켓 | 보유 시간 | 순익 ($) | 청산 유형 |");
    println!("| :--- | :--- | :--- | :---: | :---: | :--- |");
    for t in trades.iter().rev().take(10) {
        let dt = DateTime::<Utc>::from_timestamp_millis(t.entry_ts)
            .map(|d| d.format("%m-%d %H:%M").to_string())
            .unwrap_or_default();
        let laggards_str = t.laggards.join("+");
        println!("| {} | {:<8} (+{:.2}%) | {:<20} | {:>2}분 | ${:>6.2} | {:<12} |",
                 dt, t.leader, t.leader_ret * 100.0, laggards_str, t.holding_bars, t.net_pnl_usd, t.exit_reason);
    }
}
