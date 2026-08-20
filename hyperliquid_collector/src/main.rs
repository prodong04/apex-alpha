mod types;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use std::env;
use std::fs::{create_dir_all, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::{interval, sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{error, info, warn};
use types::{SubscriptionType, WsMessage, WsSubscriptionRequest};

const HL_WS_URL: &str = "wss://api.hyperliquid.xyz/ws";
const DEFAULT_COIN: &str = "BTC";
const DEFAULT_DATA_DIR: &str = "data/live";
const PING_INTERVAL_SECS: u64 = 20;
const READ_TIMEOUT_SECS: u64 = 30;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let target_coin = env::var("TARGET_COIN").unwrap_or_else(|_| DEFAULT_COIN.to_string());
    let data_dir = env::var("DATA_DIR").unwrap_or_else(|_| DEFAULT_DATA_DIR.to_string());

    info!("🚀 Starting Hyperliquid Real-Time L2 & Tick Collector...");
    info!("🎯 Target Coin: {} | Storage Directory: {}", target_coin, data_dir);

    create_dir_all(&data_dir)?;

    let total_trades = Arc::new(AtomicU64::new(0));
    let total_books = Arc::new(AtomicU64::new(0));

    loop {
        info!("🔌 Connecting to Hyperliquid WebSocket: {}", HL_WS_URL);
        match run_collector(&target_coin, &data_dir, total_trades.clone(), total_books.clone()).await {
            Ok(_) => warn!("WebSocket stream ended gracefully. Reconnecting in 3s..."),
            Err(e) => error!("WebSocket collector error: {}. Reconnecting in 3s...", e),
        }
        sleep(Duration::from_secs(3)).await;
    }
}

async fn run_collector(
    target_coin: &str,
    data_dir: &str,
    total_trades: Arc<AtomicU64>,
    total_books: Arc<AtomicU64>,
) -> Result<()> {
    let url = url::Url::parse(HL_WS_URL)?;
    let (ws_stream, response) = connect_async(url.as_str())
        .await
        .context("Failed to connect to Hyperliquid WebSocket endpoint")?;

    info!("Connected successfully! (HTTP {})", response.status());
    let (mut write, mut read) = ws_stream.split();

    // 1. Subscribe to L2 Book Depth
    let sub_l2 = WsSubscriptionRequest {
        method: "subscribe".to_string(),
        subscription: SubscriptionType::L2Book {
            coin: target_coin.to_string(),
        },
    };
    write
        .send(Message::Text(serde_json::to_string(&sub_l2)?))
        .await?;
    info!("✅ Subscribed to {} L2 Orderbook Depth", target_coin);

    // 2. Subscribe to Real-Time Trades Stream
    let sub_trades = WsSubscriptionRequest {
        method: "subscribe".to_string(),
        subscription: SubscriptionType::Trades {
            coin: target_coin.to_string(),
        },
    };
    write
        .send(Message::Text(serde_json::to_string(&sub_trades)?))
        .await?;
    info!("✅ Subscribed to {} Real-Time Trades Stream", target_coin);

    // 3. Prepare File Writers
    let coin_lower = target_coin.to_lowercase();
    let trades_file_path = format!("{}/{}_trades_live.csv", data_dir, coin_lower);
    let is_new_trades_file = !Path::new(&trades_file_path).exists();
    let trades_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&trades_file_path)?;
    let mut trades_writer = BufWriter::new(trades_file);

    if is_new_trades_file {
        writeln!(
            trades_writer,
            "timestamp_ms,datetime_utc,coin,side,price,size,hash"
        )?;
        trades_writer.flush()?;
    }

    let l2_file_path = format!("{}/{}_l2book_live.csv", data_dir, coin_lower);
    let is_new_l2_file = !Path::new(&l2_file_path).exists();
    let l2_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&l2_file_path)?;
    let mut l2_writer = BufWriter::new(l2_file);

    if is_new_l2_file {
        writeln!(
            l2_writer,
            "timestamp_ms,datetime_utc,coin,best_bid_px,best_bid_sz,best_ask_px,best_ask_sz,spread,bid_depth_levels,ask_depth_levels"
        )?;
        l2_writer.flush()?;
    }

    let mut last_log_time = Instant::now();
    let mut last_flush_time = Instant::now();
    let mut ping_ticker = interval(Duration::from_secs(PING_INTERVAL_SECS));

    loop {
        tokio::select! {
            // Heartbeat: Send ping periodically to maintain connection through cloud proxies
            _ = ping_ticker.tick() => {
                let ping_msg = serde_json::json!({"method": "ping"}).to_string();
                if let Err(e) = write.send(Message::Text(ping_msg)).await {
                    warn!("Failed to send ping heartbeat: {}", e);
                    break;
                }
            }

            // Message Receiver with Read Timeout
            msg_opt = timeout(Duration::from_secs(READ_TIMEOUT_SECS), read.next()) => {
                match msg_opt {
                    Ok(Some(Ok(msg))) => {
                        match msg {
                            Message::Text(text) => {
                                match serde_json::from_str::<WsMessage>(&text) {
                                    Ok(WsMessage::Trades { data }) => {
                                        for trade in data {
                                            let dt = DateTime::from_timestamp_millis(trade.time as i64)
                                                .unwrap_or_else(Utc::now);
                                            let dt_str = dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

                                            writeln!(
                                                trades_writer,
                                                "{},{},{},{},{},{},{}",
                                                trade.time,
                                                dt_str,
                                                trade.coin,
                                                trade.side,
                                                trade.px,
                                                trade.sz,
                                                trade.hash
                                            )?;

                                            total_trades.fetch_add(1, Ordering::Relaxed);
                                        }
                                    }
                                    Ok(WsMessage::L2Book { data }) => {
                                        let (bids, asks) = &data.levels;
                                        let best_bid = bids.first();
                                        let best_ask = asks.first();

                                        if let (Some(bid), Some(ask)) = (best_bid, best_ask) {
                                            let bid_px: f64 = bid.px.parse().unwrap_or(0.0);
                                            let ask_px: f64 = ask.px.parse().unwrap_or(0.0);
                                            let spread = ask_px - bid_px;

                                            let dt = DateTime::from_timestamp_millis(data.time as i64)
                                                .unwrap_or_else(Utc::now);
                                            let dt_str = dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

                                            writeln!(
                                                l2_writer,
                                                "{},{},{},{},{},{},{},{:.2},{},{}",
                                                data.time,
                                                dt_str,
                                                data.coin,
                                                bid.px,
                                                bid.sz,
                                                ask.px,
                                                ask.sz,
                                                spread,
                                                bids.len(),
                                                asks.len()
                                            )?;

                                            total_books.fetch_add(1, Ordering::Relaxed);

                                            if last_log_time.elapsed() >= Duration::from_secs(2) {
                                                info!(
                                                    "⚡ [LIVE {}] Best Bid: {} (${}) | Best Ask: {} (${}) | Spread: ${:.2} | Saved Trades: {} | Saved Books: {}",
                                                    data.coin,
                                                    bid.px,
                                                    bid.sz,
                                                    ask.px,
                                                    ask.sz,
                                                    spread,
                                                    total_trades.load(Ordering::Relaxed),
                                                    total_books.load(Ordering::Relaxed)
                                                );
                                                last_log_time = Instant::now();
                                            }
                                        }
                                    }
                                    Ok(WsMessage::SubscriptionResponse { data }) => {
                                        info!("Subscription confirmed: {:?}", data);
                                    }
                                    _ => {}
                                }
                            }
                            Message::Ping(payload) => {
                                let _ = write.send(Message::Pong(payload)).await;
                            }
                            Message::Close(reason) => {
                                warn!("WebSocket closed by server: {:?}", reason);
                                break;
                            }
                            _ => {}
                        }

                        if last_flush_time.elapsed() >= Duration::from_secs(1) {
                            let _ = trades_writer.flush();
                            let _ = l2_writer.flush();
                            last_flush_time = Instant::now();
                        }
                    }
                    Ok(Some(Err(e))) => {
                        error!("WebSocket read error: {}", e);
                        break;
                    }
                    Ok(None) => {
                        warn!("WebSocket stream ended (None).");
                        break;
                    }
                    Err(_) => {
                        warn!("WebSocket read timeout ({}s). Reconnecting...", READ_TIMEOUT_SECS);
                        break;
                    }
                }
            }
        }
    }

    let _ = trades_writer.flush();
    let _ = l2_writer.flush();
    Ok(())
}
