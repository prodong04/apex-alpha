use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

use crate::engine::types::UiEvent;
use crate::server::UiBroadcaster;
use crate::MarketFeedEvent;

const HL_WS_URL: &str = "wss://api.hyperliquid.xyz/ws";
const READ_TIMEOUT_SECS: u64 = 30;

pub async fn run_hyperliquid_loop(
    coin: String,
    total_trades: Arc<AtomicU64>,
    total_depth: Arc<AtomicU64>,
    total_metrics: Arc<AtomicU64>,
    broadcaster: Option<UiBroadcaster>,
    feed_tx: Option<mpsc::Sender<MarketFeedEvent>>,
    is_running: Arc<AtomicBool>,
) {
    let mut backoff_secs = 1;
    while is_running.load(Ordering::SeqCst) {
        info!("🔌 [Hyperliquid] Connecting WebSocket for {}...", coin);
        let start_time = Instant::now();

        if let Err(e) = run_hl_ws_collector(
            &coin,
            total_trades.clone(),
            total_depth.clone(),
            total_metrics.clone(),
            broadcaster.clone(),
            feed_tx.clone(),
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
    info!("🏁 [Hyperliquid] Collector stopped.");
}

async fn run_hl_ws_collector(
    coin: &str,
    total_trades: Arc<AtomicU64>,
    total_depth: Arc<AtomicU64>,
    total_metrics: Arc<AtomicU64>,
    broadcaster: Option<UiBroadcaster>,
    feed_tx: Option<mpsc::Sender<MarketFeedEvent>>,
    is_running: Arc<AtomicBool>,
) -> Result<()> {
    let (ws_stream, response) = connect_async(HL_WS_URL)
        .await
        .with_context(|| format!("Failed to connect to Hyperliquid WS: {}", HL_WS_URL))?;

    info!("⚡ [Hyperliquid] Connected! HTTP Status: {}", response.status());
    let (mut write, mut read) = ws_stream.split();

    // 1. Subscriptions (L2 Book, Trades, ActiveAssetCtx)
    let sub_trades = serde_json::json!({ "method": "subscribe", "subscription": { "type": "trades", "coin": coin } });
    let sub_l2 = serde_json::json!({ "method": "subscribe", "subscription": { "type": "l2Book", "coin": coin } });
    let sub_ctx = serde_json::json!({ "method": "subscribe", "subscription": { "type": "activeAssetCtx", "coin": coin } });

    write.send(tokio_tungstenite::tungstenite::protocol::Message::Text(sub_trades.to_string())).await?;
    write.send(tokio_tungstenite::tungstenite::protocol::Message::Text(sub_l2.to_string())).await?;
    write.send(tokio_tungstenite::tungstenite::protocol::Message::Text(sub_ctx.to_string())).await?;
    info!("✅ [Hyperliquid] Subscriptions sent for {} (L2Book, Trades, ActiveAssetCtx)", coin);

    while is_running.load(Ordering::SeqCst) {
        match timeout(Duration::from_secs(READ_TIMEOUT_SECS), read.next()).await {
            Ok(Some(Ok(msg))) => {
                if let tokio_tungstenite::tungstenite::protocol::Message::Text(text) = msg {
                    if let Ok(v) = serde_json::from_str::<Value>(&text) {
                        if let Some(channel) = v.get("channel").and_then(|c| c.as_str()) {
                            let now_ms = chrono::Utc::now().timestamp_millis();

                            match channel {
                                "trades" => {
                                    if let Some(data) = v.get("data").and_then(|d| d.as_array()) {
                                        for t in data {
                                            total_trades.fetch_add(1, Ordering::Relaxed);
                                            let p: f64 = t.get("px").and_then(|x| x.as_str()).and_then(|x| x.parse().ok()).unwrap_or(0.0);
                                            let sz: f64 = t.get("sz").and_then(|x| x.as_str()).and_then(|x| x.parse().ok()).unwrap_or(0.0);
                                            let side_str = t.get("side").and_then(|x| x.as_str()).unwrap_or("B");
                                            let is_buy = side_str == "B";
                                            let trade_time = t.get("time").and_then(|x| x.as_i64()).unwrap_or(now_ms);

                                            if let Some(ref tx) = feed_tx {
                                                let _ = tx.try_send(MarketFeedEvent::HyperliquidTrade {
                                                    timestamp: trade_time,
                                                    price: p,
                                                    qty: sz,
                                                    is_buy,
                                                });
                                            }

                                            if let Some(ref bc) = broadcaster {
                                                bc.send(UiEvent::ExchangeTrade {
                                                    timestamp: trade_time,
                                                    exchange: "Hyperliquid".to_string(),
                                                    symbol: coin.to_string(),
                                                    side: if is_buy { "BUY".to_string() } else { "SELL".to_string() },
                                                    price: p,
                                                    qty: sz,
                                                });
                                            }
                                        }
                                    }
                                }
                                "l2Book" => {
                                    total_depth.fetch_add(1, Ordering::Relaxed);
                                    if let Some(levels) = v.get("data").and_then(|d| d.get("levels")).and_then(|l| l.as_array()) {
                                        let best_bid = levels.first().and_then(|bids| bids.as_array()).and_then(|b| b.first()).and_then(|l| l.get("px")).and_then(|x| x.as_str()).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0);
                                        let best_ask = levels.get(1).and_then(|asks| asks.as_array()).and_then(|a| a.first()).and_then(|l| l.get("px")).and_then(|x| x.as_str()).and_then(|x| x.parse::<f64>().ok()).unwrap_or(0.0);

                                        if best_bid > 0.0 && best_ask > 0.0 {
                                            if let Some(ref tx) = feed_tx {
                                                let _ = tx.try_send(MarketFeedEvent::HyperliquidBook {
                                                    timestamp: now_ms,
                                                    best_bid,
                                                    best_ask,
                                                });
                                            }
                                        }
                                    }
                                }
                                "activeAssetCtx" => {
                                    total_metrics.fetch_add(1, Ordering::Relaxed);
                                    if let Some(ref tx) = feed_tx {
                                        let _ = tx.try_send(MarketFeedEvent::HyperliquidBook {
                                            timestamp: now_ms,
                                            best_bid: 0.0,
                                            best_ask: 0.0,
                                        });
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Ok(Some(Err(e))) => {
                error!("[Hyperliquid] WS read error: {}", e);
                break;
            }
            Ok(None) => {
                warn!("[Hyperliquid] Stream closed by server.");
                break;
            }
            Err(_) => {
                warn!("[Hyperliquid] WS timeout ({}s), reconnecting...", READ_TIMEOUT_SECS);
                break;
            }
        }
    }

    Ok(())
}
