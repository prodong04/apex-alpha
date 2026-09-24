use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub fn opposite(&self) -> Self {
        match self {
            Side::Buy => Side::Sell,
            Side::Sell => Side::Buy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Market,
    Limit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Liquidity {
    Maker,
    Taker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    New,
    Pending,
    PartiallyFilled,
    Filled,
    Canceled,
    TimedOut,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookLevel {
    pub price: f64,
    pub qty: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Orderbook {
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MarketState {
    pub symbol: String,
    pub timestamp: i64,
    pub last_trade_price: f64,
    pub last_trade_qty: f64,
    pub best_bid: f64,
    pub best_ask: f64,
    pub mid_price: f64,
    pub mark_price: f64,
    pub funding_rate: f64,
    pub open_interest: f64,
    pub book: Orderbook,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub price: Option<f64>,
    pub qty: f64,
    pub filled_qty: f64,
    pub avg_fill_price: f64,
    pub status: OrderStatus,
    pub timestamp: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub id: String,
    pub trade_id: String,
    pub order_id: String,
    pub symbol: String,
    pub side: Side,
    pub price: f64,
    pub qty: f64,
    pub fee: f64,
    pub liquidity: Liquidity,
    pub realized_pnl: f64,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitOrder {
    pub order_id: u64,
    pub symbol: String,
    pub side: Side,
    pub price: f64,
    pub qty: f64,
    pub filled_qty: f64,
    pub placed_ts: i64,
    pub timeout_ms: i64,
    pub status: OrderStatus,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderFill {
    pub timestamp: i64,
    pub order_id: u64,
    pub symbol: String,
    pub side: Side,
    pub price: f64,
    pub qty: f64,
    pub fee: f64,
    pub realized_pnl: f64,
    pub is_maker: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub side: Option<Side>,
    pub size: f64,
    pub entry_price: f64,
    pub unrealized_pnl: f64,
    pub roe_pct: f64,
    pub liquidation_price: f64,
    pub margin: f64,
    pub leverage: f64,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            symbol: "BTC".to_string(),
            side: None,
            size: 0.0,
            entry_price: 0.0,
            unrealized_pnl: 0.0,
            roe_pct: 0.0,
            liquidation_price: 0.0,
            margin: 0.0,
            leverage: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub balance: f64,
    pub equity: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
    pub net_pnl: f64,
    pub total_fees: f64,
    pub total_volume: f64,
    pub win_trades: u64,
    pub loss_trades: u64,
    pub win_rate: f64,
    pub total_trades: u64,
}

impl Account {
    pub fn new(initial_balance: f64) -> Self {
        Self {
            balance: initial_balance,
            equity: initial_balance,
            unrealized_pnl: 0.0,
            realized_pnl: 0.0,
            net_pnl: 0.0,
            total_fees: 0.0,
            total_volume: 0.0,
            win_trades: 0,
            loss_trades: 0,
            win_rate: 0.0,
            total_trades: 0,
        }
    }

    pub fn update_equity(&mut self, unrealized_pnl: f64) {
        self.unrealized_pnl = unrealized_pnl;
        self.equity = self.balance + unrealized_pnl;
        self.net_pnl = self.equity - 10000.0;
        let total = self.win_trades + self.loss_trades;
        self.total_trades = total;
        if total > 0 {
            self.win_rate = (self.win_trades as f64 / total as f64) * 100.0;
        }
    }
}

pub type AccountState = Account;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyDecision {
    pub timestamp: i64,
    pub symbol: String,
    pub action: String,
    pub side: Option<Side>,
    pub price: f64,
    pub qty: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum UiEvent {
    #[serde(rename = "market_tick")]
    MarketTick {
        timestamp: i64,
        symbol: String,
        price: f64,
        best_bid: f64,
        best_ask: f64,
        mark_price: f64,
        funding_rate: f64,
        open_interest: f64,
    },
    #[serde(rename = "exchange_trade")]
    ExchangeTrade {
        timestamp: i64,
        exchange: String,
        symbol: String,
        side: String,
        price: f64,
        qty: f64,
    },
    #[serde(rename = "order_placed")]
    OrderPlaced {
        timestamp: i64,
        order_id: u64,
        symbol: String,
        side: Side,
        price: f64,
        qty: f64,
        timeout_ms: i64,
        reason: String,
    },
    #[serde(rename = "order_fill")]
    OrderFill(OrderFill),
    #[serde(rename = "order_canceled")]
    OrderCanceled {
        timestamp: i64,
        order_id: u64,
        reason: String,
    },
    #[serde(rename = "account_update")]
    AccountUpdate {
        timestamp: i64,
        balance: f64,
        equity: f64,
        net_pnl: f64,
        realized_pnl: f64,
        unrealized_pnl: f64,
        total_fees: f64,
        position_size: f64,
        position_side: Option<Side>,
        entry_price: f64,
        roe_pct: f64,
        liquidation_price: f64,
        win_rate: f64,
        total_trades: u64,
    },
    #[serde(rename = "decision_marker")]
    DecisionMarker {
        timestamp: i64,
        symbol: String,
        action: String,
        price: f64,
        qty: f64,
        reason: String,
    },
    #[serde(rename = "system_status")]
    SystemStatus {
        timestamp: i64,
        live_mode: bool,
        strategy_name: String,
        collector_status: String,
        active_orders_count: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MarketFeedEvent {
    BinanceSpotTick { timestamp: i64, price: f64, qty: f64, is_buy: bool },
    HyperliquidTrade { timestamp: i64, price: f64, qty: f64, is_buy: bool },
    HyperliquidBook { timestamp: i64, best_bid: f64, best_ask: f64 },
}
