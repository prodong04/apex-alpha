use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use std::fs::{create_dir_all, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::{interval, sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info, warn};

use crate::types::{HlSubscriptionType, HlWsMessage, HlWsSubscriptionRequest};

const HL_WS_URL: &str = "wss://api.hyperliquid.xyz/ws";
const PING_INTERVAL_SECS: u64 = 20;
const READ_TIMEOUT_SECS: u64 = 30;

pub async fn run_hyperliquid_loop(
    coin: String,
    data_dir: String,
    total_trades: Arc<AtomicU64>,
    total_books: Arc<AtomicU64>,
    total_metrics: Arc<AtomicU64>,
    is_running: Arc<AtomicBool>,
) {
    let hl_dir = format!("{}/hyperliquid", data_dir);
    if let Err(e) = create_dir_all(&hl_dir) {
        error!("Failed to create Hyperliquid data directory {}: {}", hl_dir, e);
        return;
    }

    let mut backoff_secs = 1;
    while is_running.load(Ordering::SeqCst) {
        info!("🔌 [Hyperliquid] Connecting WebSocket for {}...", coin);
        let start_time = Instant::now();

        if let Err(e) = run_hl_collector(
            &coin,
            &hl_dir,
            total_trades.clone(),
            total_books.clone(),
            total_metrics.clone(),
            is_running.clone(),
        )
        .await
        {
            if !is_running.load(Ordering::SeqCst) {
                break;
            }
            error!("❌ [Hyperliquid] Stream error: {:#}", e);
        }

        if start_time.elapsed() > Duration::from_secs(60) {
            backoff_secs = 1;
        } else {
            backoff_secs = (backoff_secs * 2).min(30);
        }

        if is_running.load(Ordering::SeqCst) {
            warn!("⏳ [Hyperliquid] Reconnecting in {}s...", backoff_secs);
            sleep(Duration::from_secs(backoff_secs)).await;
        }
    }
    info!("🏁 [Hyperliquid] WebSocket collector stopped.");
}

async fn run_hl_collector(
    coin: &str,
    hl_dir: &str,
    total_trades: Arc<AtomicU64>,
    total_books: Arc<AtomicU64>,
    total_metrics: Arc<AtomicU64>,
    is_running: Arc<AtomicBool>,
) -> Result<()> {
    let (ws_stream, response) = connect_async(HL_WS_URL)
        .await
        .context("Failed to connect to Hyperliquid WebSocket")?;

    info!("⚡ [Hyperliquid] Connected! HTTP Status: {}", response.status());
    let (mut write, mut read) = ws_stream.split();

    // 1. Subscribe to L2 Book Depth (20 levels)
    let sub_l2 = HlWsSubscriptionRequest {
        method: "subscribe".to_string(),
        subscription: HlSubscriptionType::L2Book {
            coin: coin.to_string(),
        },
    };
    write
        .send(Message::Text(serde_json::to_string(&sub_l2)?))
        .await?;

    // 2. Subscribe to Real-Time Trades
    let sub_trades = HlWsSubscriptionRequest {
        method: "subscribe".to_string(),
        subscription: HlSubscriptionType::Trades {
            coin: coin.to_string(),
        },
    };
    write
        .send(Message::Text(serde_json::to_string(&sub_trades)?))
        .await?;

    // 3. Subscribe to Active Asset Context
    let sub_metrics = HlWsSubscriptionRequest {
        method: "subscribe".to_string(),
        subscription: HlSubscriptionType::ActiveAssetCtx {
            coin: coin.to_string(),
        },
    };
    write
        .send(Message::Text(serde_json::to_string(&sub_metrics)?))
        .await?;

    info!("✅ [Hyperliquid] Subscriptions registered for {}", coin);

    let coin_lower = coin.to_lowercase();

    // L2 Book Writer
    let l2_path = format!("{}/{}_depth20_live.csv", hl_dir, coin_lower);
    let is_new_l2 = !Path::new(&l2_path).exists();
    let l2_file = OpenOptions::new().create(true).append(true).open(&l2_path)?;
    let mut l2_writer = BufWriter::new(l2_file);
    if is_new_l2 {
        writeln!(
            l2_writer,
            "timestamp_ms,datetime_utc,coin,best_bid_px,best_bid_sz,best_ask_px,best_ask_sz,spread,bids,asks"
        )?;
        l2_writer.flush()?;
    }

    // Trades Writer
    let trades_path = format!("{}/{}_trades_live.csv", hl_dir, coin_lower);
    let is_new_trades = !Path::new(&trades_path).exists();
    let trades_file = OpenOptions::new().create(true).append(true).open(&trades_path)?;
    let mut trades_writer = BufWriter::new(trades_file);
    if is_new_trades {
        writeln!(
            trades_writer,
            "timestamp_ms,datetime_utc,coin,side,price,size,hash,tid"
        )?;
        trades_writer.flush()?;
    }

    // Metrics Writer
    let metrics_path = format!("{}/{}_metrics_live.csv", hl_dir, coin_lower);
    let is_new_metrics = !Path::new(&metrics_path).exists();
    let metrics_file = OpenOptions::new().create(true).append(true).open(&metrics_path)?;
    let mut metrics_writer = BufWriter::new(metrics_file);
    if is_new_metrics {
        writeln!(
            metrics_writer,
            "timestamp_ms,datetime_utc,coin,funding_rate,open_interest,oracle_px,mark_px,mid_px,premium,impact_bid_px,impact_ask_px,day_ntl_vlm"
        )?;
        metrics_writer.flush()?;
    }

    let mut last_flush_time = Instant::now();
    let mut ping_ticker = interval(Duration::from_secs(PING_INTERVAL_SECS));

    while is_running.load(Ordering::SeqCst) {
        tokio::select! {
            _ = ping_ticker.tick() => {
                let ping_msg = serde_json::json!({"method": "ping"}).to_string();
                if let Err(e) = write.send(Message::Text(ping_msg)).await {
                    warn!("[Hyperliquid] Failed to send ping heartbeat: {}", e);
                    break;
                }
            }

            msg_opt = timeout(Duration::from_secs(READ_TIMEOUT_SECS), read.next()) => {
                match msg_opt {
                    Ok(Some(Ok(msg))) => {
                        match msg {
                            Message::Text(text) => {
                                match serde_json::from_str::<HlWsMessage>(&text) {
                                    Ok(HlWsMessage::Trades { data }) => {
                                        for trade in data {
                                            let dt = DateTime::from_timestamp_millis(trade.time as i64)
                                                .unwrap_or_else(Utc::now);
                                            let dt_str = dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

                                            writeln!(
                                                trades_writer,
                                                "{},{},{},{},{},{},{},{}",
                                                trade.time,
                                                dt_str,
                                                trade.coin,
                                                trade.side,
                                                trade.px,
                                                trade.sz,
                                                trade.hash,
                                                trade.tid
                                            )?;
                                            total_trades.fetch_add(1, Ordering::Relaxed);
                                        }
                                    }
                                    Ok(HlWsMessage::L2Book { data }) => {
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

                                            let bids_json = serde_json::to_string(bids).unwrap_or_else(|_| "[]".to_string());
                                            let asks_json = serde_json::to_string(asks).unwrap_or_else(|_| "[]".to_string());

                                            writeln!(
                                                l2_writer,
                                                "{},{},{},{},{},{},{},{:.2},\"{}\",\"{}\"",
                                                data.time,
                                                dt_str,
                                                data.coin,
                                                bid.px,
                                                bid.sz,
                                                ask.px,
                                                ask.sz,
                                                spread,
                                                bids_json.replace('"', "\"\""),
                                                asks_json.replace('"', "\"\"")
                                            )?;
                                            total_books.fetch_add(1, Ordering::Relaxed);
                                        }
                                    }
                                    Ok(HlWsMessage::ActiveAssetCtx { data }) => {
                                        let now = Utc::now();
                                        let ts = now.timestamp_millis();
                                        let dt_str = now.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
                                        let ctx = &data.ctx;

                                        let impact_bid = ctx.impact_pxs.as_ref().and_then(|p| p.get(0)).map(|s| s.as_str()).unwrap_or("");
                                        let impact_ask = ctx.impact_pxs.as_ref().and_then(|p| p.get(1)).map(|s| s.as_str()).unwrap_or("");
                                        let mid_px = ctx.mid_px.as_deref().unwrap_or("");
                                        let premium = ctx.premium.as_deref().unwrap_or("");

                                        writeln!(
                                            metrics_writer,
                                            "{},{},{},{},{},{},{},{},{},{},{},{}",
                                            ts,
                                            dt_str,
                                            data.coin,
                                            ctx.funding,
                                            ctx.open_interest,
                                            ctx.oracle_px,
                                            ctx.mark_px,
                                            mid_px,
                                            premium,
                                            impact_bid,
                                            impact_ask,
                                            ctx.day_ntl_vlm
                                        )?;
                                        total_metrics.fetch_add(1, Ordering::Relaxed);
                                    }
                                    Ok(HlWsMessage::SubscriptionResponse { data }) => {
                                        info!("[Hyperliquid] Subscription confirmed: {:?}", data);
                                    }
                                    Ok(HlWsMessage::Pong) => {
                                        debug!("[Hyperliquid] Pong received.");
                                    }
                                    Ok(HlWsMessage::Unknown) => {}
                                    Err(e) => {
                                        warn!("[Hyperliquid] JSON error: {}. Raw: {}", e, text);
                                    }
                                }
                            }
                            Message::Ping(payload) => {
                                let _ = write.send(Message::Pong(payload)).await;
                            }
                            Message::Close(reason) => {
                                warn!("[Hyperliquid] WS closed: {:?}", reason);
                                break;
                            }
                            _ => {}
                        }

                        if last_flush_time.elapsed() >= Duration::from_secs(1) {
                            let _ = l2_writer.flush();
                            let _ = trades_writer.flush();
                            let _ = metrics_writer.flush();
                            last_flush_time = Instant::now();
                        }
                    }
                    Ok(Some(Err(e))) => {
                        error!("[Hyperliquid] Read error: {}", e);
                        break;
                    }
                    Ok(None) => {
                        warn!("[Hyperliquid] Stream ended.");
                        break;
                    }
                    Err(_) => {
                        warn!("[Hyperliquid] Read timeout ({}s), reconnecting...", READ_TIMEOUT_SECS);
                        break;
                    }
                }
            }
        }
    }

    let _ = l2_writer.flush();
    let _ = trades_writer.flush();
    let _ = metrics_writer.flush();
    Ok(())
}
