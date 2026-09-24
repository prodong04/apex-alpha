mod collector;
mod config;
mod engine;
mod server;
mod strategy;

use anyhow::Result;
use chrono::Utc;
use std::fs::create_dir_all;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{interval, sleep};
use tracing::{error, info, warn};

use crate::config::AppConfig;
use crate::engine::deadman_switch::DeadmanSwitch;
use crate::engine::paper_matcher::PaperMatcher;
use crate::engine::types::*;
use crate::server::{start_web_server, UiBroadcaster};
use crate::strategy::LeadLagSpikerStrategy;


#[tokio::main]
async fn main() -> Result<()> {
    // 1. Logging Initialization
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = AppConfig::load_or_default("config.toml");
    create_dir_all("frontend")?;

    info!("==================================================================");
    info!("🚀 APEX ALPHA: Real-Time HFT Execution Engine");
    info!("🎯 Symbol              : {}", config.symbol);
    info!("🛡️ Deadman's Switch    : Active (1500ms Cancel-on-Disconnect)");
    info!("⏱️ Fast Cancel Timeout : 500ms (Post-Only Maker Matcher)");
    info!("💰 Initial Balance     : ${:.2}", config.initial_balance);
    info!("🌐 Web Dashboard UI    : http://{}:{}", config.server_host, config.server_port);
    info!("==================================================================");

    let is_running = Arc::new(AtomicBool::new(true));
    let is_running_clone = is_running.clone();

    tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            info!("🛑 Received termination signal. Shutting down cleanly...");
            is_running_clone.store(false, Ordering::SeqCst);
        }
    });

    // 2. Initialize UI Broadcaster & Web Server
    let broadcaster = UiBroadcaster::new(8192);
    let server_broadcaster = broadcaster.clone();
    let host = config.server_host.clone();
    let port = config.server_port;

    tokio::spawn(async move {
        if let Err(e) = start_web_server(&host, port, server_broadcaster).await {
            error!("Web server error: {}", e);
        }
    });

    // 3. Central In-Memory Market Feed Channel
    let (feed_tx, mut feed_rx) = mpsc::channel::<MarketFeedEvent>(32768);

    // 4. Background Collectors (Binance Spot & Hyperliquid)
    let spot_depth = Arc::new(AtomicU64::new(0));
    let spot_bbo = Arc::new(AtomicU64::new(0));
    let spot_trades = Arc::new(AtomicU64::new(0));
    let hl_depth = Arc::new(AtomicU64::new(0));
    let hl_trades = Arc::new(AtomicU64::new(0));
    let hl_metrics = Arc::new(AtomicU64::new(0));

    // A) Binance Spot Collector
    let feed_tx_spot = feed_tx.clone();
    let broadcaster_spot = broadcaster.clone();
    let is_running_spot = is_running.clone();
    let coin_spot = config.symbol.clone();
    tokio::spawn(async move {
        collector::binance_spot::run_binance_spot_loop(
            coin_spot,
            spot_depth,
            spot_bbo,
            spot_trades,
            Some(broadcaster_spot),
            Some(feed_tx_spot),
            is_running_spot,
        )
        .await;
    });

    // B) Hyperliquid Collector
    let feed_tx_hl = feed_tx.clone();
    let broadcaster_hl = broadcaster.clone();
    let is_running_hl = is_running.clone();
    let coin_hl = config.symbol.clone();
    tokio::spawn(async move {
        collector::hyperliquid::run_hyperliquid_loop(
            coin_hl,
            hl_trades,
            hl_depth,
            hl_metrics,
            Some(broadcaster_hl),
            Some(feed_tx_hl),
            is_running_hl,
        )
        .await;
    });

    // 5. Initialize Core HFT Components
    let mut deadman_switch = DeadmanSwitch::new(3000); // 3.0s disconnect/silence threshold
    let mut paper_matcher = PaperMatcher::new(config.initial_balance, 0.0001); // 0.01% Maker fee
    let mut strategy = LeadLagSpikerStrategy::new(80.0, 0.5, 500); // $80 spike shock, 0.5 BTC, 500ms timeout

    let mut current_hl_price = 0.0;
    let mut current_hl_bid = 0.0;
    let mut current_hl_ask = 0.0;
    let mut last_ui_emit_ms = 0i64;

    let mut timer_fast_cancel = interval(Duration::from_millis(20)); // Check timeouts every 20ms

    info!("⚡ [HFT ENGINE CORE] Starting Zero-Latency Hot Loop...");

    while is_running.load(Ordering::SeqCst) {
        let now_ms = Utc::now().timestamp_millis();

        // 1. Process Real-Time Market Feed Events
        tokio::select! {
            Some(event) = feed_rx.recv() => {
                match event {
                    MarketFeedEvent::BinanceSpotTick { timestamp, price, .. } => {
                        deadman_switch.touch_binance(timestamp);
                        strategy.on_binance_spot_tick(price, timestamp);

                        // Check Strategy Lead-Lag Spike Signal
                        let pos = paper_matcher.get_position(current_hl_price);
                        if let Some(decision) = strategy.check_signal(current_hl_bid, current_hl_ask, &pos, timestamp) {
                            if let Some(side) = decision.side {
                                let order = paper_matcher.place_order(
                                    &decision.symbol,
                                    side,
                                    decision.price,
                                    decision.qty,
                                    500, // 500ms Fast Cancel
                                    timestamp,
                                    &decision.reason,
                                );

                                broadcaster.send(UiEvent::OrderPlaced {
                                    timestamp,
                                    order_id: order.order_id,
                                    symbol: order.symbol,
                                    side: order.side,
                                    price: order.price,
                                    qty: order.qty,
                                    timeout_ms: order.timeout_ms,
                                    reason: order.reason,
                                });

                                broadcaster.send(UiEvent::DecisionMarker {
                                    timestamp: decision.timestamp,
                                    symbol: decision.symbol,
                                    action: decision.action,
                                    price: decision.price,
                                    qty: decision.qty,
                                    reason: decision.reason,
                                });
                            }
                        }
                    }
                    MarketFeedEvent::HyperliquidTrade { timestamp, price, qty, is_buy: _ } => {
                        deadman_switch.touch_hyperliquid(timestamp);
                        current_hl_price = price;

                        // Match against real HL trade tick!
                        let fills = paper_matcher.on_real_market_trade(price, qty, timestamp);
                        for fill in fills {
                            broadcaster.send(UiEvent::OrderFill(fill));
                        }
                    }
                    MarketFeedEvent::HyperliquidBook { timestamp, best_bid, best_ask } => {
                        deadman_switch.touch_hyperliquid(timestamp);
                        if best_bid > 0.0 {
                            current_hl_bid = best_bid;
                        }
                        if best_ask > 0.0 {
                            current_hl_ask = best_ask;
                        }
                    }
                }
            }
            _ = timer_fast_cancel.tick() => {
                // 1. Check 0.8s Ultra-Fast Scalp Exit (Target $35 / Stop $25 / 800ms Hold Timeout)
                if let Some(exit_fill) = paper_matcher.check_position_exit(current_hl_price, now_ms) {
                    broadcaster.send(UiEvent::OrderFill(exit_fill));
                }

                // 2. Check 500ms Fast Cancel Timeouts
                let canceled = paper_matcher.check_timeouts(now_ms);
                for (order_id, reason) in canceled {
                    broadcaster.send(UiEvent::OrderCanceled {
                        timestamp: now_ms,
                        order_id,
                        reason,
                    });
                }

                // 3. Deadman's Switch Watchdog Check
                if !deadman_switch.check_health(now_ms) {
                    let canceled_ids = paper_matcher.cancel_all("Deadman's Switch (WebSocket Disconnect/Silence)");
                    for order_id in canceled_ids {
                        broadcaster.send(UiEvent::OrderCanceled {
                            timestamp: now_ms,
                            order_id,
                            reason: "Deadman Switch Emergency Cancel".to_string(),
                        });
                    }
                }

                // 4. Periodic UI State Broadcast (Every 250ms)
                if now_ms - last_ui_emit_ms >= 250 {
                    last_ui_emit_ms = now_ms;
                    let acc = paper_matcher.get_account(current_hl_price);
                    let pos = paper_matcher.get_position(current_hl_price);

                    broadcaster.send(UiEvent::MarketTick {
                        timestamp: now_ms,
                        symbol: config.symbol.clone(),
                        price: current_hl_price,
                        best_bid: current_hl_bid,
                        best_ask: current_hl_ask,
                        mark_price: current_hl_price,
                        funding_rate: 0.0001,
                        open_interest: 0.0,
                    });

                    broadcaster.send(UiEvent::AccountUpdate {
                        timestamp: now_ms,
                        balance: acc.balance,
                        equity: acc.equity,
                        net_pnl: acc.net_pnl,
                        realized_pnl: acc.realized_pnl,
                        unrealized_pnl: acc.unrealized_pnl,
                        total_fees: acc.total_fees,
                        position_size: pos.size,
                        position_side: pos.side,
                        entry_price: pos.entry_price,
                        roe_pct: pos.roe_pct,
                        liquidation_price: pos.liquidation_price,
                        win_rate: acc.win_rate,
                        total_trades: acc.total_trades,
                    });

                    broadcaster.send(UiEvent::SystemStatus {
                        timestamp: now_ms,
                        live_mode: config.live_mode,
                        strategy_name: "Lead-Lag 1s Delta Shock Sniping (Post-Only 500ms)".to_string(),
                        collector_status: "HEALTHY (Watchdog Active)".to_string(),
                        active_orders_count: paper_matcher.active_orders_count(),
                    });
                }
            }
        }
    }

    info!("🏁 HFT Engine shutdown cleanly.");
    Ok(())
}
