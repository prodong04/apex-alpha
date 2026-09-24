use apex_engine::dtw::clustering::{cluster_into_baskets, compute_dtw_distance_matrix, ClusterBasket};
use apex_engine::dtw::distance::dtw_distance;
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};

struct TradeRecord {
    entry_ts: i64,
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

    let mut valid_timestamps: Vec<i64> = all_timestamps.into_iter().collect();
    let symbols: Vec<String> = raw_data.keys().cloned().collect();
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
    let (symbols, timestamps, price_series) = load_klines(data_dir);
    let total_bars = timestamps.len();

    let lookback_bars = 60; // 60 mins lookback
    let recluster_interval = 20; // Re-cluster every 20 mins
    let num_clusters = 5; // 5 tighter dynamic baskets
    let dtw_window_radius = 6; // 6-min warping window

    let leader_window = 5; // 5-minute momentum check
    let leader_momentum_threshold = 0.012; // Leader +1.2% strong breakout
    let laggard_max_return = 0.003; // Laggard <= +0.3%
    let max_laggards_per_basket = 2; // TOP 2 most DTW-similar laggards only!

    let target_take_profit = 0.010; // +1.0% take profit
    let stop_loss = -0.007; // -0.7% stop loss
    let max_holding_bars = 20; // 20 mins timeout
    let fee_rate = 0.0006; // 0.06% taker fee
    let basket_capital_usd = 1000.0;

    let mut current_baskets: Vec<ClusterBasket> = Vec::new();
    let mut current_dist_matrix: Vec<Vec<f64>> = Vec::new();
    let mut trades: Vec<TradeRecord> = Vec::new();
    let mut last_entry_bar: HashMap<usize, usize> = HashMap::new();

    let btc_symbol = "BTCUSDT".to_string();

    for bar_idx in lookback_bars..(total_bars - max_holding_bars) {
        // 1. Periodically Re-Cluster using DTW
        if (bar_idx - lookback_bars) % recluster_interval == 0 {
            let mut sub_series = Vec::new();
            for s in &symbols {
                let p = &price_series[s][(bar_idx - lookback_bars)..bar_idx];
                sub_series.push(p.to_vec());
            }

            current_dist_matrix = compute_dtw_distance_matrix(&symbols, &sub_series, dtw_window_radius);
            current_baskets = cluster_into_baskets(&symbols, &current_dist_matrix, num_clusters);
        }

        // BTC Trend Filter: Don't long alt laggards if BTC is dumping (> -0.4% in 15m)
        if let Some(btc_prices) = price_series.get(&btc_symbol) {
            let btc_ret_15m = (btc_prices[bar_idx] - btc_prices[bar_idx - 15]) / btc_prices[bar_idx - 15];
            if btc_ret_15m < -0.004 {
                continue;
            }
        }

        // 2. Scan each basket
        for basket in &current_baskets {
            if basket.symbols.len() < 2 { continue; }

            if let Some(&last_bar) = last_entry_bar.get(&basket.cluster_id) {
                if bar_idx < last_bar + 15 { continue; }
            }

            // Detect Leader in basket
            let mut leader_symbol = None;
            let mut leader_ret = 0.0;

            for s in &basket.symbols {
                let p_curr = price_series[s][bar_idx];
                let p_prev = price_series[s][bar_idx - leader_window];
                let ret = (p_curr - p_prev) / p_prev;

                if ret >= leader_momentum_threshold && ret > leader_ret {
                    leader_ret = ret;
                    leader_symbol = Some(s.clone());
                }
            }

            if let Some(leader) = leader_symbol {
                let leader_idx = symbols.iter().position(|s| s == &leader).unwrap();

                // Candidate laggards sorted by DTW similarity to Leader
                let mut candidate_laggards: Vec<(String, f64)> = Vec::new(); // (symbol, dtw_dist)

                for s in &basket.symbols {
                    if s == &leader { continue; }
                    let p_curr = price_series[s][bar_idx];
                    let p_prev = price_series[s][bar_idx - leader_window];
                    let ret = (p_curr - p_prev) / p_prev;

                    if ret <= laggard_max_return {
                        let s_idx = symbols.iter().position(|sym| sym == s).unwrap();
                        let dist = current_dist_matrix[leader_idx][s_idx];
                        candidate_laggards.push((s.clone(), dist));
                    }
                }

                // Sort by DTW distance ascending (closest waveform shape to leader)
                candidate_laggards.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                let selected_laggards: Vec<String> = candidate_laggards
                    .into_iter()
                    .take(max_laggards_per_basket)
                    .map(|(s, _)| s)
                    .collect();

                if !selected_laggards.is_empty() {
                    let entry_ts = timestamps[bar_idx];
                    let mut entry_prices: HashMap<String, f64> = HashMap::new();
                    for lag in &selected_laggards {
                        entry_prices.insert(lag.clone(), price_series[lag][bar_idx]);
                    }

                    let mut exit_bar = bar_idx + max_holding_bars;
                    let mut exit_reason = "TIMEOUT_20M".to_string();
                    let mut exit_ret = 0.0;

                    for forward_bar in (bar_idx + 1)..=(bar_idx + max_holding_bars) {
                        let mut basket_ret = 0.0;
                        for lag in &selected_laggards {
                            let p_fwd = price_series[lag][forward_bar];
                            let p_ent = entry_prices[lag];
                            basket_ret += (p_fwd - p_ent) / p_ent;
                        }
                        basket_ret /= selected_laggards.len() as f64;

                        if basket_ret >= target_take_profit {
                            exit_bar = forward_bar;
                            exit_reason = "TAKE_PROFIT".to_string();
                            exit_ret = basket_ret;
                            break;
                        } else if basket_ret <= stop_loss {
                            exit_bar = forward_bar;
                            exit_reason = "STOP_LOSS".to_string();
                            exit_ret = basket_ret;
                            break;
                        }

                        if forward_bar == bar_idx + max_holding_bars {
                            exit_ret = basket_ret;
                        }
                    }

                    let holding_bars = exit_bar - bar_idx;
                    let net_return = exit_ret - fee_rate;
                    let net_pnl_usd = basket_capital_usd * net_return;

                    trades.push(TradeRecord {
                        entry_ts,
                        leader,
                        leader_ret,
                        laggards: selected_laggards,
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
    println!("💎 HIGH-CONVICTION DTW TOP-2 LAGGARD SPILLOVER REPORT");
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
    let to_count = trades.iter().filter(|t| t.exit_reason == "TIMEOUT_20M").count();

    println!("------------------------------------------------------------------");
    println!("🏆 PERFORMANCE METRICS:");
    println!("  - Win Rate (승률)       : 🔥 {:.1}% ({} Wins / {} Losses)", win_rate, wins, losses);
    println!("  - Total Net PnL (순익)  : 🔥 +${:.2} (기본 바스켓당 $1,000 기준)", total_net_pnl);
    println!("  - Profit Factor (손익비): 🔥 {:.2}", profit_factor);
    println!("  - Avg Holding Time      : {:.1} mins", avg_holding);
    println!("------------------------------------------------------------------");
    println!("🎯 EXIT BREAKDOWN:");
    println!("  - 🟢 Take Profit (+1.0%) : {:>2} trades ({:.1}%)", tp_count, (tp_count as f64 / trades.len() as f64)*100.0);
    println!("  - 🔴 Stop Loss (-0.7%)   : {:>2} trades ({:.1}%)", sl_count, (sl_count as f64 / trades.len() as f64)*100.0);
    println!("  - ⏱️ 20m Timeout Exit    : {:>2} trades ({:.1}%)", to_count, (to_count as f64 / trades.len() as f64)*100.0);
    println!("==================================================================");

    println!("\n🔍 SAMPLE SPILLOVER BASKET TRADES:");
    println!("| 진입 일시 (UTC) | 리더 코인 (5분 돌파) | Top-2 래거 바스켓 | 보유 시간 | 순익 ($) | 청산 유형 |");
    println!("| :--- | :--- | :--- | :---: | :---: | :--- |");
    for t in trades.iter().rev().take(10) {
        let dt = DateTime::<Utc>::from_timestamp_millis(t.entry_ts)
            .map(|d| d.format("%m-%d %H:%M").to_string())
            .unwrap_or_default();
        let laggards_str = t.laggards.join(" + ");
        println!("| {} | {:<8} (+{:.2}%) | {:<22} | {:>2}분 | ${:>6.2} | {:<12} |",
                 dt, t.leader, t.leader_ret * 100.0, laggards_str, t.holding_bars, t.net_pnl_usd, t.exit_reason);
    }
}
