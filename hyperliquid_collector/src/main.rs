mod types;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use std::env;
use std::fs::{create_dir_all, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::{interval, sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info, warn};
use types::{SubscriptionType, WsMessage, WsSubscriptionRequest};

const HL_WS_URL: &str = "wss://api.hyperliquid.xyz/ws";
const DEFAULT_COIN: &str = "BTC";
const DEFAULT_DATA_DIR: &str = "data/live";
const PING_INTERVAL_SECS: u64 = 20;
const READ_TIMEOUT_SECS: u64 = 30;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let target_coin = env::var("TARGET_COIN").unwrap_or_else(|_| DEFAULT_COIN.to_string());
    let data_dir = env::var("DATA_DIR").unwrap_or_else(|_| DEFAULT_DATA_DIR.to_string());

    info!("============================================================");
    info!("🚀 Hyperliquid Production Market Data Collector");
    info!("🎯 Target Symbol : {}", target_coin);
    info!("📁 Output Dir    : {}", data_dir);
    info!("💓 Ping Interval : {}s | ⏱️ Read Timeout: {}s", PING_INTERVAL_SECS, READ_TIMEOUT_SECS);
    info!("============================================================");

    create_dir_all(&data_dir)?;

    let is_running = Arc::new(AtomicBool::new(true));
    let is_running_clone = is_running.clone();

    // 2. Setup graceful shutdown for Docker / AWS SIGTERM / SIGINT
    tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            info!("🛑 Received termination signal (SIGINT/SIGTERM). Shutting down cleanly...");
            is_running_clone.store(false, Ordering::SeqCst);
        }
    });

    let total_trades = Arc::new(AtomicU64::new(0));
    let total_books = Arc::new(AtomicU64::new(0));
    let mut retry_backoff_secs: u64 = 1;

    while is_running.load(Ordering::SeqCst) {
        info!("🔌 Connecting to Hyperliquid WebSocket: {}", HL_WS_URL);
        
        let start_time = Instant::now();
        match run_collector(
            &target_coin,
            &data_dir,
            total_trades.clone(),
            total_books.clone(),
            is_running.clone(),
        )
        .await
        {
            Ok(_) => {
                if !is_running.load(Ordering::SeqCst) {
                    break;
                }
                warn!("WebSocket connection closed gracefully.");
            }
            Err(e) => {
                if !is_running.load(Ordering::SeqCst) {
                    break;
                }
                error!("WebSocket collector error: {:#}", e);
            }
        }

        // If connected for more than 60s, reset backoff to 1s
        if start_time.elapsed() > Duration::from_secs(60) {
            retry_backoff_secs = 1;
        } else {
            retry_backoff_secs = (retry_backoff_secs * 2).min(30);
        }

        if is_running.load(Ordering::SeqCst) {
            warn!("⏳ Reconnecting in {}s (exponential backoff)...", retry_backoff_secs);
            sleep(Duration::from_secs(retry_backoff_secs)).await;
        }
    }

    info!("🏁 Collector gracefully exited with all buffers flushed. Goodbye!");
    Ok(())
}

async fn run_collector(
    target_coin: &str,
    data_dir: &str,
    total_trades: Arc<AtomicU64>,
    total_books: Arc<AtomicU64>,
    is_running: Arc<AtomicBool>,
) -> Result<()> {
    let url = url::Url::parse(HL_WS_URL)?;
    let (ws_stream, response) = connect_async(url.as_str())
        .await
        .context("Failed to establish WebSocket handshake with Hyperliquid")?;

    info!("⚡ Connected! Handshake Status: {}", response.status());
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
    info!("✅ Subscribed: {} L2 Orderbook Depth", target_coin);

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
    info!("✅ Subscribed: {} Real-Time Trades (Ticks)", target_coin);

    // 3. Prepare Append File Writers
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

    while is_running.load(Ordering::SeqCst) {
        tokio::select! {
            // Heartbeat: Proactive Client Ping to keep connection alive through cloud NAT/proxies
            _ = ping_ticker.tick() => {
                let ping_msg = serde_json::json!({"method": "ping"}).to_string();
                if let Err(e) = write.send(Message::Text(ping_msg)).await {
                    warn!("Failed to send ping heartbeat: {}", e);
                    break;
                }
                debug!("Heartbeat ping sent to Hyperliquid.");
            }

            // Message Receiver with Read Timeout to prevent dead/half-open socket hangs
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
                                    Ok(WsMessage::Pong) => {
                                        debug!("Pong response received from Hyperliquid.");
                                    }
                                    Ok(WsMessage::Unknown) => {
                                        debug!("Received unhandled WebSocket payload: {}", text);
                                    }
                                    Err(e) => {
                                        warn!("JSON deserialization error: {}. Raw: {}", e, text);
                                    }
                                }
                            }
                            Message::Ping(payload) => {
                                let _ = write.send(Message::Pong(payload)).await;
                            }
                            Message::Pong(_) => {
                                debug!("Protocol Pong received.");
                            }
                            Message::Close(reason) => {
                                warn!("WebSocket connection closed by server: {:?}", reason);
                                break;
                            }
                            _ => {}
                        }

                        // Periodic flush every 1 second
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
                        warn!("WebSocket stream ended (server disconnected).");
                        break;
                    }
                    Err(_) => {
                        warn!("WebSocket read timeout ({}s). Socket dead, triggering reconnect...", READ_TIMEOUT_SECS);
                        break;
                    }
                }
            }
        }
    }

    info!("Flushing file buffers before disconnecting...");
    let _ = trades_writer.flush();
    let _ = l2_writer.flush();
    Ok(())
}
