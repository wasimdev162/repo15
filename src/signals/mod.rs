pub mod microstructure;

pub use microstructure::{MicrostructureSignals, SignalCalculator};

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

use crate::md::{OrderBook, Trade};

/// Signal engine that continuously computes trading signals
pub struct SignalEngine {
    calculator: SignalCalculator,
    orderbook: Arc<RwLock<OrderBook>>,
    trades: Arc<RwLock<Vec<Trade>>>,
    latest_signals: Arc<RwLock<MicrostructureSignals>>,
}

impl SignalEngine {
    pub fn new(
        orderbook: Arc<RwLock<OrderBook>>,
        trades: Arc<RwLock<Vec<Trade>>>,
    ) -> Self {
        Self {
            calculator: SignalCalculator::new(),
            orderbook,
            trades,
            latest_signals: Arc::new(RwLock::new(MicrostructureSignals::default())),
        }
    }

    pub fn get_signals(&self) -> Arc<RwLock<MicrostructureSignals>> {
        Arc::clone(&self.latest_signals)
    }

    /// Start the signal calculation loop
    pub async fn start(mut self, update_interval_ms: u64) -> Result<()> {
        let mut ticker = interval(Duration::from_millis(update_interval_ms));

        loop {
            ticker.tick().await;

            // Get current market state
            let book = self.orderbook.read().await;
            let trades = self.trades.read().await;

            // Compute signals
            let signals = self.calculator.compute_signals(&book, &trades);

            // Update shared state
            let mut latest = self.latest_signals.write().await;
            *latest = signals.clone();
            drop(latest);

            // Record book update for stuffing detection
            self.calculator.record_book_update(book.timestamp);

            // Record trades for stuffing detection
            for trade in trades.iter() {
                self.calculator.record_trade(trade.timestamp);
            }
        }
    }
}
