use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use reqwest::Client;
use std::fs::{create_dir_all, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::{interval, sleep, timeout};
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

use crate::types::{
    BinanceAggTrade, BinanceBookTicker, BinanceCombinedStream, BinanceForceOrderWrapper,
    BinanceFuturesDepth20, BinanceLongShortRatio, BinanceMarkPrice, BinanceOpenInterest,
    BinanceTakerRatio,
};

const FUTURES_PUBLIC_STREAM: &str = "wss://fstream.binance.com/stream";
const FUTURES_MARKET_STREAM: &str = "wss://fstream.binance.com/market/stream";
const READ_TIMEOUT_SECS: u64 = 30;
const REST_BASE_URL: &str = "https://fapi.binance.com";

pub async fn run_binance_futures_loop(
    coin: String,
    data_dir: String,
    total_depth: Arc<AtomicU64>,
    total_bbo: Arc<AtomicU64>,
    total_trades: Arc<AtomicU64>,
    total_mark: Arc<AtomicU64>,
    total_liqs: Arc<AtomicU64>,
    total_metrics: Arc<AtomicU64>,
    is_running: Arc<AtomicBool>,
) {
    let futures_dir = format!("{}/binance_futures", data_dir);
    if let Err(e) = create_dir_all(&futures_dir) {
        error!("Failed to create Binance Futures data directory {}: {}", futures_dir, e);
        return;
    }

    // 1. REST Metrics Poller Task
    let coin_for_rest = coin.clone();
    let dir_for_rest = futures_dir.clone();
    let is_running_rest = is_running.clone();
    let total_metrics_clone = total_metrics.clone();
    tokio::spawn(async move {
        run_futures_rest_poller(coin_for_rest, dir_for_rest, total_metrics_clone, is_running_rest).await;
    });

    // 2. Market Trades, Mark Price & Liquidations Task
    let coin_for_market = coin.clone();
    let dir_for_market = futures_dir.clone();
    let is_running_market = is_running.clone();
    let total_trades_clone = total_trades.clone();
    let total_mark_clone = total_mark.clone();
    let total_liqs_clone = total_liqs.clone();
    tokio::spawn(async move {
        run_futures_market_loop(
            coin_for_market,
            dir_for_market,
            total_trades_clone,
            total_mark_clone,
            total_liqs_clone,
            is_running_market,
        )
        .await;
    });

    // 3. Orderbook & BBO Task (Main Loop for this module)
    let mut backoff_secs = 1;
    while is_running.load(Ordering::SeqCst) {
        info!("🔌 [Binance Futures Public] Connecting Orderbook/BBO WebSocket for {}...", coin);
        let start_time = Instant::now();

        if let Err(e) = run_futures_depth_collector(
            &coin,
            &futures_dir,
            total_depth.clone(),
            total_bbo.clone(),
            is_running.clone(),
        )
        .await
        {
            if !is_running.load(Ordering::SeqCst) {
                break;
            }
            error!("❌ [Binance Futures Public] Stream error: {:#}", e);
        }

        if start_time.elapsed() > Duration::from_secs(60) {
            backoff_secs = 1;
        } else {
            backoff_secs = (backoff_secs * 2).min(30);
        }

        if is_running.load(Ordering::SeqCst) {
            warn!("⏳ [Binance Futures Public] Reconnecting in {}s...", backoff_secs);
            sleep(Duration::from_secs(backoff_secs)).await;
        }
    }
    info!("🏁 [Binance Futures Public] Collector stopped.");
}

async fn run_futures_depth_collector(
    coin: &str,
    futures_dir: &str,
    total_depth: Arc<AtomicU64>,
    total_bbo: Arc<AtomicU64>,
    is_running: Arc<AtomicBool>,
) -> Result<()> {
    let coin_lower = coin.to_lowercase();
    let symbol = format!("{}usdt", coin_lower);

    let ws_url = format!(
        "{}?streams={}@depth20@100ms/{}@bookTicker",
        FUTURES_PUBLIC_STREAM, symbol, symbol
    );

    let (ws_stream, response) = connect_async(&ws_url)
        .await
        .with_context(|| format!("Failed to connect to Binance Futures Public WS: {}", ws_url))?;

    info!("⚡ [Binance Futures Public] Connected! HTTP Status: {}", response.status());
    let (_write, mut read) = ws_stream.split();

    // Depth20 Writer
    let depth_path = format!("{}/{}_depth20_live.csv", futures_dir, coin_lower);
    let is_new_depth = !Path::new(&depth_path).exists();
    let depth_file = OpenOptions::new().create(true).append(true).open(&depth_path)?;
    let mut depth_writer = BufWriter::new(depth_file);
    if is_new_depth {
        writeln!(
            depth_writer,
            "event_time_ms,transact_time_ms,local_ts,local_datetime_utc,coin,first_update_id,last_update_id,best_bid_px,best_bid_sz,best_ask_px,best_ask_sz,spread,bids,asks"
        )?;
        depth_writer.flush()?;
    }

    // BBO Writer
    let bbo_path = format!("{}/{}_bbo_live.csv", futures_dir, coin_lower);
    let is_new_bbo = !Path::new(&bbo_path).exists();
    let bbo_file = OpenOptions::new().create(true).append(true).open(&bbo_path)?;
    let mut bbo_writer = BufWriter::new(bbo_file);
    if is_new_bbo {
        writeln!(
            bbo_writer,
            "event_time_ms,transact_time_ms,local_ts,local_datetime_utc,coin,update_id,bid_px,bid_sz,ask_px,ask_sz,spread"
        )?;
        bbo_writer.flush()?;
    }

    let mut last_flush_time = Instant::now();

    while is_running.load(Ordering::SeqCst) {
        match timeout(Duration::from_secs(READ_TIMEOUT_SECS), read.next()).await {
            Ok(Some(Ok(msg))) => {
                if let tokio_tungstenite::tungstenite::protocol::Message::Text(text) = msg {
                    let now = Utc::now();
                    let local_ts = now.timestamp_millis();
                    let local_dt_str = now.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

                    if text.contains("@depth20") {
                        if let Ok(wrapper) = serde_json::from_str::<BinanceCombinedStream<BinanceFuturesDepth20>>(&text) {
                            let data = wrapper.data;
                            let best_bid = data.bids.first();
                            let best_ask = data.asks.first();

                            let (bid_px, bid_sz) = best_bid.map(|b| (b[0].as_str(), b[1].as_str())).unwrap_or(("0", "0"));
                            let (ask_px, ask_sz) = best_ask.map(|a| (a[0].as_str(), a[1].as_str())).unwrap_or(("0", "0"));
                            let b_px: f64 = bid_px.parse().unwrap_or(0.0);
                            let a_px: f64 = ask_px.parse().unwrap_or(0.0);
                            let spread = a_px - b_px;

                            let bids_json = serde_json::to_string(&data.bids).unwrap_or_else(|_| "[]".to_string());
                            let asks_json = serde_json::to_string(&data.asks).unwrap_or_else(|_| "[]".to_string());

                            writeln!(
                                depth_writer,
                                "{},{},{},{},{},{},{},{},{},{},{},{:.2},\"{}\",\"{}\"",
                                data.event_time,
                                data.transact_time,
                                local_ts,
                                local_dt_str,
                                coin,
                                data.first_update_id,
                                data.last_update_id,
                                bid_px,
                                bid_sz,
                                ask_px,
                                ask_sz,
                                spread,
                                bids_json.replace('"', "\"\""),
                                asks_json.replace('"', "\"\"")
                            )?;
                            total_depth.fetch_add(1, Ordering::Relaxed);
                        }
                    } else if text.contains("@bookTicker") {
                        if let Ok(wrapper) = serde_json::from_str::<BinanceCombinedStream<BinanceBookTicker>>(&text) {
                            let data = wrapper.data;
                            let b_px: f64 = data.best_bid_px.parse().unwrap_or(0.0);
                            let a_px: f64 = data.best_ask_px.parse().unwrap_or(0.0);
                            let spread = a_px - b_px;

                            writeln!(
                                bbo_writer,
                                "{},{},{},{},{},{},{},{},{},{},{:.2}",
                                data.event_time,
                                data.transact_time,
                                local_ts,
                                local_dt_str,
                                coin,
                                data.update_id,
                                data.best_bid_px,
                                data.best_bid_qty,
                                data.best_ask_px,
                                data.best_ask_qty,
                                spread
                            )?;
                            total_bbo.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }

                if last_flush_time.elapsed() >= Duration::from_secs(1) {
                    let _ = depth_writer.flush();
                    let _ = bbo_writer.flush();
                    last_flush_time = Instant::now();
                }
            }
            Ok(Some(Err(e))) => {
                error!("[Binance Futures Public] WS read error: {}", e);
                break;
            }
            Ok(None) => {
                warn!("[Binance Futures Public] Stream closed by server.");
                break;
            }
            Err(_) => {
                warn!("[Binance Futures Public] WS timeout ({}s), reconnecting...", READ_TIMEOUT_SECS);
                break;
            }
        }
    }

    let _ = depth_writer.flush();
    let _ = bbo_writer.flush();
    Ok(())
}

async fn run_futures_market_loop(
    coin: String,
    futures_dir: String,
    total_trades: Arc<AtomicU64>,
    total_mark: Arc<AtomicU64>,
    total_liqs: Arc<AtomicU64>,
    is_running: Arc<AtomicBool>,
) {
    let mut backoff_secs = 1;
    while is_running.load(Ordering::SeqCst) {
        info!("🔌 [Binance Futures Market] Connecting Trades/Mark/Liqs WebSocket for {}...", coin);
        let start_time = Instant::now();

        if let Err(e) = run_futures_market_collector(
            &coin,
            &futures_dir,
            total_trades.clone(),
            total_mark.clone(),
            total_liqs.clone(),
            is_running.clone(),
        )
        .await
        {
            if !is_running.load(Ordering::SeqCst) {
                break;
            }
            error!("❌ [Binance Futures Market] Stream error: {:#}", e);
        }

        if start_time.elapsed() > Duration::from_secs(60) {
            backoff_secs = 1;
        } else {
            backoff_secs = (backoff_secs * 2).min(30);
        }

        if is_running.load(Ordering::SeqCst) {
            warn!("⏳ [Binance Futures Market] Reconnecting in {}s...", backoff_secs);
            sleep(Duration::from_secs(backoff_secs)).await;
        }
    }
    info!("🏁 [Binance Futures Market] Collector stopped.");
}

async fn run_futures_market_collector(
    coin: &str,
    futures_dir: &str,
    total_trades: Arc<AtomicU64>,
    total_mark: Arc<AtomicU64>,
    total_liqs: Arc<AtomicU64>,
    is_running: Arc<AtomicBool>,
) -> Result<()> {
    let coin_lower = coin.to_lowercase();
    let symbol = format!("{}usdt", coin_lower);

    let ws_url = format!(
        "{}?streams={}@aggTrade/{}@markPrice@1s/!forceOrder@arr",
        FUTURES_MARKET_STREAM, symbol, symbol
    );

    let (ws_stream, response) = connect_async(&ws_url)
        .await
        .with_context(|| format!("Failed to connect to Binance Futures Market WS: {}", ws_url))?;

    info!("⚡ [Binance Futures Market] Connected! HTTP Status: {}", response.status());
    let (_write, mut read) = ws_stream.split();

    // 1. AggTrades Writer
    let trades_path = format!("{}/{}_aggtrades_live.csv", futures_dir, coin_lower);
    let is_new_trades = !Path::new(&trades_path).exists();
    let trades_file = OpenOptions::new().create(true).append(true).open(&trades_path)?;
    let mut trades_writer = BufWriter::new(trades_file);
    if is_new_trades {
        writeln!(
            trades_writer,
            "agg_trade_id,trade_time_ms,event_time_ms,local_ts,trade_datetime_utc,coin,price,qty,first_trade_id,last_trade_id,is_buyer_maker"
        )?;
        trades_writer.flush()?;
    }

    // 2. Mark Price & Funding Writer
    let mark_path = format!("{}/{}_mark_funding_live.csv", futures_dir, coin_lower);
    let is_new_mark = !Path::new(&mark_path).exists();
    let mark_file = OpenOptions::new().create(true).append(true).open(&mark_path)?;
    let mut mark_writer = BufWriter::new(mark_file);
    if is_new_mark {
        writeln!(
            mark_writer,
            "event_time_ms,local_ts,event_datetime_utc,coin,mark_px,index_px,est_settle_px,funding_rate,next_funding_time_ms"
        )?;
        mark_writer.flush()?;
    }

    // 3. Liquidations Writer
    let liqs_path = format!("{}/{}_liquidations_live.csv", futures_dir, coin_lower);
    let is_new_liqs = !Path::new(&liqs_path).exists();
    let liqs_file = OpenOptions::new().create(true).append(true).open(&liqs_path)?;
    let mut liqs_writer = BufWriter::new(liqs_file);
    if is_new_liqs {
        writeln!(
            liqs_writer,
            "event_time_ms,trade_time_ms,local_ts,event_datetime_utc,coin,side,order_type,time_in_force,orig_qty,price,avg_price,order_status,last_filled_qty,accum_filled_qty"
        )?;
        liqs_writer.flush()?;
    }

    let mut last_flush_time = Instant::now();

    while is_running.load(Ordering::SeqCst) {
        match timeout(Duration::from_secs(READ_TIMEOUT_SECS), read.next()).await {
            Ok(Some(Ok(msg))) => {
                if let tokio_tungstenite::tungstenite::protocol::Message::Text(text) = msg {
                    let now = Utc::now();
                    let local_ts = now.timestamp_millis();
                    let _local_dt_str = now.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

                    if text.contains("@aggTrade") {
                        if let Ok(wrapper) = serde_json::from_str::<BinanceCombinedStream<BinanceAggTrade>>(&text) {
                            let data = wrapper.data;
                            let dt = DateTime::from_timestamp_millis(data.trade_time as i64)
                                .unwrap_or_else(Utc::now);
                            let dt_str = dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

                            writeln!(
                                trades_writer,
                                "{},{},{},{},{},{},{},{},{},{},{}",
                                data.agg_trade_id,
                                data.trade_time,
                                data.event_time,
                                local_ts,
                                dt_str,
                                coin,
                                data.price,
                                data.quantity,
                                data.first_trade_id,
                                data.last_trade_id,
                                data.is_buyer_maker
                            )?;
                            total_trades.fetch_add(1, Ordering::Relaxed);
                        }
                    } else if text.contains("@markPrice") {
                        if let Ok(wrapper) = serde_json::from_str::<BinanceCombinedStream<BinanceMarkPrice>>(&text) {
                            let data = wrapper.data;
                            let dt = DateTime::from_timestamp_millis(data.event_time as i64)
                                .unwrap_or_else(Utc::now);
                            let dt_str = dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

                            writeln!(
                                mark_writer,
                                "{},{},{},{},{},{},{},{},{}",
                                data.event_time,
                                local_ts,
                                dt_str,
                                coin,
                                data.mark_price,
                                data.index_price,
                                data.est_settle_price,
                                data.funding_rate,
                                data.next_funding_time
                            )?;
                            total_mark.fetch_add(1, Ordering::Relaxed);
                        }
                    } else if text.contains("forceOrder") {
                        if let Ok(wrapper) = serde_json::from_str::<BinanceCombinedStream<BinanceForceOrderWrapper>>(&text) {
                            let data = wrapper.data;
                            let order = data.order;
                            let dt = DateTime::from_timestamp_millis(data.event_time as i64)
                                .unwrap_or_else(Utc::now);
                            let dt_str = dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

                            writeln!(
                                liqs_writer,
                                "{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                                data.event_time,
                                order.trade_time,
                                local_ts,
                                dt_str,
                                coin,
                                order.side,
                                order.order_type,
                                order.time_in_force,
                                order.orig_qty,
                                order.price,
                                order.avg_price,
                                order.order_status,
                                order.last_filled_qty,
                                order.accum_filled_qty
                            )?;
                            total_liqs.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }

                if last_flush_time.elapsed() >= Duration::from_secs(1) {
                    let _ = trades_writer.flush();
                    let _ = mark_writer.flush();
                    let _ = liqs_writer.flush();
                    last_flush_time = Instant::now();
                }
            }
            Ok(Some(Err(e))) => {
                error!("[Binance Futures Market] WS read error: {}", e);
                break;
            }
            Ok(None) => {
                warn!("[Binance Futures Market] Stream closed by server.");
                break;
            }
            Err(_) => {
                warn!("[Binance Futures Market] WS timeout ({}s), reconnecting...", READ_TIMEOUT_SECS);
                break;
            }
        }
    }

    let _ = trades_writer.flush();
    let _ = mark_writer.flush();
    let _ = liqs_writer.flush();
    Ok(())
}

async fn run_futures_rest_poller(
    coin: String,
    futures_dir: String,
    total_metrics: Arc<AtomicU64>,
    is_running: Arc<AtomicBool>,
) {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let coin_lower = coin.to_lowercase();
    let symbol = format!("{}USDT", coin.to_uppercase());
    let metrics_path = format!("{}/{}_metrics_live.csv", futures_dir, coin_lower);
    let is_new = !Path::new(&metrics_path).exists();
    let metrics_file = match OpenOptions::new().create(true).append(true).open(&metrics_path) {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to open futures metrics CSV: {}", e);
            return;
        }
    };
    let mut writer = BufWriter::new(metrics_file);

    if is_new {
        let _ = writeln!(
            writer,
            "timestamp_ms,datetime_utc,coin,open_interest,top_trader_long_short_pos_ratio,top_trader_long_pos,top_trader_short_pos,top_trader_long_short_acc_ratio,global_long_short_acc_ratio,taker_buy_sell_vol_ratio,taker_buy_vol,taker_sell_vol"
        );
        let _ = writer.flush();
    }

    let mut ticker = interval(Duration::from_secs(10));
    info!("📊 [Binance Futures REST] Metrics Poller started (10s interval for OI & Sentiment ratios).");

    while is_running.load(Ordering::SeqCst) {
        ticker.tick().await;
        if !is_running.load(Ordering::SeqCst) {
            break;
        }

        let now = Utc::now();
        let ts = now.timestamp_millis();
        let dt_str = now.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

        // 1. Open Interest
        let oi_url = format!("{}/fapi/v1/openInterest?symbol={}", REST_BASE_URL, symbol);
        let oi_res = client.get(&oi_url).send().await;
        let mut open_interest = String::new();
        if let Ok(res) = oi_res {
            if let Ok(oi_data) = res.json::<BinanceOpenInterest>().await {
                open_interest = oi_data.open_interest;
            }
        }

        // 2. Top Trader Position Ratio
        let top_pos_url = format!(
            "{}/futures/data/topLongShortPositionRatio?symbol={}&period=5m&limit=1",
            REST_BASE_URL, symbol
        );
        let top_pos_res = client.get(&top_pos_url).send().await;
        let mut top_pos_ratio = String::new();
        let mut top_pos_long = String::new();
        let mut top_pos_short = String::new();
        if let Ok(res) = top_pos_res {
            if let Ok(arr) = res.json::<Vec<BinanceLongShortRatio>>().await {
                if let Some(first) = arr.first() {
                    top_pos_ratio = first.long_short_ratio.clone();
                    top_pos_long = first.long_account.clone();
                    top_pos_short = first.short_account.clone();
                }
            }
        }

        // 3. Top Trader Account Ratio
        let top_acc_url = format!(
            "{}/futures/data/topLongShortAccountRatio?symbol={}&period=5m&limit=1",
            REST_BASE_URL, symbol
        );
        let top_acc_res = client.get(&top_acc_url).send().await;
        let mut top_acc_ratio = String::new();
        if let Ok(res) = top_acc_res {
            if let Ok(arr) = res.json::<Vec<BinanceLongShortRatio>>().await {
                if let Some(first) = arr.first() {
                    top_acc_ratio = first.long_short_ratio.clone();
                }
            }
        }

        // 4. Global Long/Short Ratio
        let global_acc_url = format!(
            "{}/futures/data/globalLongShortAccountRatio?symbol={}&period=5m&limit=1",
            REST_BASE_URL, symbol
        );
        let global_acc_res = client.get(&global_acc_url).send().await;
        let mut global_acc_ratio = String::new();
        if let Ok(res) = global_acc_res {
            if let Ok(arr) = res.json::<Vec<BinanceLongShortRatio>>().await {
                if let Some(first) = arr.first() {
                    global_acc_ratio = first.long_short_ratio.clone();
                }
            }
        }

        // 5. Taker Volume Ratio
        let taker_url = format!(
            "{}/futures/data/takerlongshortRatio?symbol={}&period=5m&limit=1",
            REST_BASE_URL, symbol
        );
        let taker_res = client.get(&taker_url).send().await;
        let mut taker_buy_sell_ratio = String::new();
        let mut taker_buy_vol = String::new();
        let mut taker_sell_vol = String::new();
        if let Ok(res) = taker_res {
            if let Ok(arr) = res.json::<Vec<BinanceTakerRatio>>().await {
                if let Some(first) = arr.first() {
                    taker_buy_sell_ratio = first.buy_sell_ratio.clone();
                    taker_buy_vol = first.buy_vol.clone();
                    taker_sell_vol = first.sell_vol.clone();
                }
            }
        }

        if !open_interest.is_empty() || !top_pos_ratio.is_empty() {
            let _ = writeln!(
                writer,
                "{},{},{},{},{},{},{},{},{},{},{},{}",
                ts,
                dt_str,
                coin,
                open_interest,
                top_pos_ratio,
                top_pos_long,
                top_pos_short,
                top_acc_ratio,
                global_acc_ratio,
                taker_buy_sell_ratio,
                taker_buy_vol,
                taker_sell_vol
            );
            let _ = writer.flush();
            total_metrics.fetch_add(1, Ordering::Relaxed);
        }
    }
}
