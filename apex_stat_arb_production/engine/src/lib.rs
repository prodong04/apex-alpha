pub mod core;
pub mod strategy;

pub use core::dtw::{dtw_distance, zscore_normalize};
pub use core::pit_universe::PitUniverseFilter;
pub use core::volatility::{compute_relative_atr, compute_volume_zscore, BarSnapshot};
pub use strategy::portfolio_book::PortfolioBook;
pub use strategy::stat_arb::{ActivePairPosition, ClosedPairTrade, StatArbSignal};
