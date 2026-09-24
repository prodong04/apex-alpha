use crate::engine::broker::Broker;
use crate::engine::types::*;

pub struct PassiveStrategy {
    name: String,
}

impl PassiveStrategy {
    pub fn new() -> Self {
        Self {
            name: "Passive (No-Op Observer)".to_string(),
        }
    }
}

impl Default for PassiveStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl super::Strategy for PassiveStrategy {
    fn name(&self) -> &str {
        &self.name
    }

    fn on_market_update(
        &mut self,
        _market: &MarketState,
        _broker: &mut dyn Broker,
    ) -> Option<StrategyDecision> {
        // Pure passive observation: No trade decisions or markers
        None
    }
}
