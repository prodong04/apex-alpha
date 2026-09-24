use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::connect_async;
use tracing::{error, info, warn};

use crate::collector::types::{BinanceAggTrade, BinanceBookTicker, BinanceSpotDepth20};
use crate::engine::types::UiEvent;
use crate::server::UiBroadcaster;
use crate::MarketFeedEvent;

const SPOT_WS_URL: &str = "wss://stream.binance.com:9443/ws";
const READ_TIMEOUT_SECS: u64 = 30;

pub async fn run_binance_spot_loop(
    coin: String,
    total_depth: Arc<AtomicU64>,
    total_bbo: Arc<AtomicU64>,
    total_trades: Arc<AtomicU64>,
    broadcaster: Option<UiBroadcaster>,
    feed_tx: Option<mpsc::Sender<MarketFeedEvent>>,
    is_running: Arc<AtomicBool>,
) {
    let mut backoff_secs = 1;
    while is_running.load(Ordering::SeqCst) {
        info!("🔌 [Binance Spot] Connecting WebSocket for {}...", coin);
        let start_time = Instant::now();

        if let Err(e) = run_spot_ws_collector(
            &coin,
            total_depth.clone(),
            total_bbo.clone(),
            total_trades.clone(),
            broadcaster.clone(),
            feed_tx.clone(),
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

async fn run_spot_ws_collector(
    coin: &str,
    total_depth: Arc<AtomicU64>,
    total_bbo: Arc<AtomicU64>,
    total_trades: Arc<AtomicU64>,
    broadcaster: Option<UiBroadcaster>,
    feed_tx: Option<mpsc::Sender<MarketFeedEvent>>,
    is_running: Arc<AtomicBool>,
) -> Result<()> {
    let coin_lower = coin.to_lowercase();
    let symbol = format!("{}usdt", coin_lower);

    let (ws_stream, response) = connect_async(SPOT_WS_URL)
        .await
        .with_context(|| format!("Failed to connect to Binance Spot WS: {}", SPOT_WS_URL))?;

    info!("⚡ [Binance Spot] Connected! HTTP Status: {}", response.status());
    let (mut write, mut read) = ws_stream.split();

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

    while is_running.load(Ordering::SeqCst) {
        match timeout(Duration::from_secs(READ_TIMEOUT_SECS), read.next()).await {
            Ok(Some(Ok(msg))) => {
                if let tokio_tungstenite::tungstenite::protocol::Message::Text(text) = msg {
                    if text.contains("\"lastUpdateId\"") {
                        if serde_json::from_str::<BinanceSpotDepth20>(&text).is_ok() {
                            total_depth.fetch_add(1, Ordering::Relaxed);
                        }
                    } else if text.contains("\"u\":") && text.contains("\"b\":") && text.contains("\"a\":") && !text.contains("\"lastUpdateId\"") {
                        if serde_json::from_str::<BinanceBookTicker>(&text).is_ok() {
                            total_bbo.fetch_add(1, Ordering::Relaxed);
                        }
                    } else if text.contains("\"e\":\"aggTrade\"") {
                        if let Ok(data) = serde_json::from_str::<BinanceAggTrade>(&text) {
                            total_trades.fetch_add(1, Ordering::Relaxed);
                            let p: f64 = data.price.parse().unwrap_or(0.0);
                            let q: f64 = data.quantity.parse().unwrap_or(0.0);
                            let is_buy = !data.is_buyer_maker;

                            if let Some(ref tx) = feed_tx {
                                let _ = tx.try_send(MarketFeedEvent::BinanceSpotTick {
                                    timestamp: data.trade_time as i64,
                                    price: p,
                                    qty: q,
                                    is_buy,
                                });
                            }

                            if let Some(ref bc) = broadcaster {
                                bc.send(UiEvent::ExchangeTrade {
                                    timestamp: data.trade_time as i64,
                                    exchange: "Binance Spot".to_string(),
                                    symbol: coin.to_string(),
                                    side: if is_buy { "BUY".to_string() } else { "SELL".to_string() },
                                    price: p,
                                    qty: q,
                                });
                            }
                        }
                    }
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

    Ok(())
}
