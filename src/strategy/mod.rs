pub mod lead_lag_spiker;
pub mod passive;

pub use lead_lag_spiker::*;
pub use passive::*;

use crate::engine::broker::Broker;
use crate::engine::types::{MarketState, StrategyDecision};

pub trait Strategy: Send + Sync {
    fn name(&self) -> &str;
    fn on_market_update(
        &mut self,
        market: &MarketState,
        broker: &mut dyn Broker,
    ) -> Option<StrategyDecision>;
}
