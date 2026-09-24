use apex_stat_arb_production::strategy::portfolio_book::PortfolioBook;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    info!("🚀 APEX ALPHA: 120H DTW ADAPTIVE STAT-ARB ENGINE INITIALIZED");
    info!("• Two Sigma Standard Institutional Market-Neutral Engine");
    info!("• Point-In-Time Universe Filter: Top 30 Liquid Tokens (7-Day Rolling)");
    info!("• Dynamic ATR Volatility Scaling: TP = 1.5*ATR_60m | SL = -1.0*ATR_60m | Max Hold = 25m");
    info!("• Portfolio Capacity: Max 5 Pair Slots (Zero Market Beta)");

    let book = PortfolioBook::new(5, 10000.0);
    info!("• Initial Portfolio Book Created: ${:.2} Capital ($2,000 / slot)", book.total_capital_usd);
    info!("• Ready for WebSocket Market Feed Connection...");

    Ok(())
}
