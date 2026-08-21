use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use std::fs::{create_dir_all, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::{sleep, timeout};
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

use crate::types::{BinanceAggTrade, BinanceBookTicker, BinanceSpotDepth20};

const SPOT_WS_URL: &str = "wss://stream.binance.com:9443/ws";
const READ_TIMEOUT_SECS: u64 = 30;

pub async fn run_binance_spot_loop(
    coin: String,
    data_dir: String,
    total_depth: Arc<AtomicU64>,
    total_bbo: Arc<AtomicU64>,
    total_trades: Arc<AtomicU64>,
    is_running: Arc<AtomicBool>,
) {
    let spot_dir = format!("{}/binance_spot", data_dir);
    if let Err(e) = create_dir_all(&spot_dir) {
        error!("Failed to create Binance Spot data directory {}: {}", spot_dir, e);
        return;
    }

    let mut backoff_secs = 1;
    while is_running.load(Ordering::SeqCst) {
        info!("🔌 [Binance Spot] Connecting WebSocket for {}...", coin);
        let start_time = Instant::now();

        if let Err(e) = run_spot_collector(
            &coin,
            &spot_dir,
            total_depth.clone(),
            total_bbo.clone(),
            total_trades.clone(),
            is_running.clone(),
        )
        .await
        {
            if !is_running.load(Ordering::SeqCst) {
                break;
            }
            error!("❌ [Binance Spot] Stream error: {:#}", e);
        }

        if start_time.elapsed() > Duration::from_secs(60) {
            backoff_secs = 1;
        } else {
            backoff_secs = (backoff_secs * 2).min(30);
        }

        if is_running.load(Ordering::SeqCst) {
            warn!("⏳ [Binance Spot] Reconnecting in {}s...", backoff_secs);
            sleep(Duration::from_secs(backoff_secs)).await;
        }
    }
    info!("🏁 [Binance Spot] Collector stopped.");
}

async fn run_spot_collector(
    coin: &str,
    spot_dir: &str,
    total_depth: Arc<AtomicU64>,
    total_bbo: Arc<AtomicU64>,
    total_trades: Arc<AtomicU64>,
    is_running: Arc<AtomicBool>,
) -> Result<()> {
    let coin_lower = coin.to_lowercase();
    let symbol = format!("{}usdt", coin_lower);

    let (ws_stream, response) = connect_async(SPOT_WS_URL)
        .await
        .with_context(|| format!("Failed to connect to Binance Spot WS: {}", SPOT_WS_URL))?;

    info!("⚡ [Binance Spot] Connected! HTTP Status: {}", response.status());
    let (mut write, mut read) = ws_stream.split();

    // Subscribe to: depth20@100ms, bookTicker, aggTrade
    let sub_payload = serde_json::json!({
        "method": "SUBSCRIBE",
        "params": [
            format!("{}@depth20@100ms", symbol),
            format!("{}@bookTicker", symbol),
            format!("{}@aggTrade", symbol),
        ],
        "id": 1
    });

    write
        .send(tokio_tungstenite::tungstenite::protocol::Message::Text(sub_payload.to_string()))
        .await?;

    info!("✅ [Binance Spot] Subscriptions sent for {} (Depth20, BBO, AggTrades)", symbol);

    // 1. Prepare Writers
    let depth_path = format!("{}/{}_depth20_live.csv", spot_dir, coin_lower);
    let is_new_depth = !Path::new(&depth_path).exists();
    let depth_file = OpenOptions::new().create(true).append(true).open(&depth_path)?;
    let mut depth_writer = BufWriter::new(depth_file);
    if is_new_depth {
        writeln!(
            depth_writer,
            "last_update_id,local_ts,local_datetime_utc,coin,best_bid_px,best_bid_sz,best_ask_px,best_ask_sz,spread,bids,asks"
        )?;
        depth_writer.flush()?;
    }

    let bbo_path = format!("{}/{}_bbo_live.csv", spot_dir, coin_lower);
    let is_new_bbo = !Path::new(&bbo_path).exists();
    let bbo_file = OpenOptions::new().create(true).append(true).open(&bbo_path)?;
    let mut bbo_writer = BufWriter::new(bbo_file);
    if is_new_bbo {
        writeln!(
            bbo_writer,
            "update_id,local_ts,local_datetime_utc,coin,bid_px,bid_sz,ask_px,ask_sz,spread"
        )?;
        bbo_writer.flush()?;
    }

    let trades_path = format!("{}/{}_aggtrades_live.csv", spot_dir, coin_lower);
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

    let mut last_flush_time = Instant::now();

    while is_running.load(Ordering::SeqCst) {
        match timeout(Duration::from_secs(READ_TIMEOUT_SECS), read.next()).await {
            Ok(Some(Ok(msg))) => {
                if let tokio_tungstenite::tungstenite::protocol::Message::Text(text) = msg {
                    let now = Utc::now();
                    let local_ts = now.timestamp_millis();
                    let local_dt_str = now.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

                    if text.contains("\"lastUpdateId\"") {
                        match serde_json::from_str::<BinanceSpotDepth20>(&text) {
                            Ok(data) => {
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
                                    "{},{},{},{},{},{},{},{},{:.2},\"{}\",\"{}\"",
                                    data.last_update_id,
                                    local_ts,
                                    local_dt_str,
                                    coin,
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
                            Err(e) => warn!("[Binance Spot] Depth parse error: {}", e),
                        }
                    } else if text.contains("\"u\":") && text.contains("\"b\":") && text.contains("\"a\":") && !text.contains("\"lastUpdateId\"") {
                        match serde_json::from_str::<BinanceBookTicker>(&text) {
                            Ok(data) => {
                                let b_px: f64 = data.best_bid_px.parse().unwrap_or(0.0);
                                let a_px: f64 = data.best_ask_px.parse().unwrap_or(0.0);
                                let spread = a_px - b_px;

                                writeln!(
                                    bbo_writer,
                                    "{},{},{},{},{},{},{},{},{:.2}",
                                    data.update_id,
                                    local_ts,
                                    local_dt_str,
                                    coin,
                                    data.best_bid_px,
                                    data.best_bid_qty,
                                    data.best_ask_px,
                                    data.best_ask_qty,
                                    spread
                                )?;
                                total_bbo.fetch_add(1, Ordering::Relaxed);
                            }
                            Err(e) => warn!("[Binance Spot] BBO parse error: {}", e),
                        }
                    } else if text.contains("\"e\":\"aggTrade\"") {
                        match serde_json::from_str::<BinanceAggTrade>(&text) {
                            Ok(data) => {
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
                            Err(e) => warn!("[Binance Spot] aggTrade parse error: {}", e),
                        }
                    }
                }

                if last_flush_time.elapsed() >= Duration::from_secs(1) {
                    let _ = depth_writer.flush();
                    let _ = bbo_writer.flush();
                    let _ = trades_writer.flush();
                    last_flush_time = Instant::now();
                }
            }
            Ok(Some(Err(e))) => {
                error!("[Binance Spot] WS read error: {}", e);
                break;
            }
            Ok(None) => {
                warn!("[Binance Spot] Stream closed by server.");
                break;
            }
            Err(_) => {
                warn!("[Binance Spot] WS timeout ({}s), reconnecting...", READ_TIMEOUT_SECS);
                break;
            }
        }
    }

    let _ = depth_writer.flush();
    let _ = bbo_writer.flush();
    let _ = trades_writer.flush();
    Ok(())
}
