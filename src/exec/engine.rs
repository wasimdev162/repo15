use anyhow::Result;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

use super::oms::OMS;
use crate::md::OrderBook;

/// Execution engine that manages order lifecycle
pub struct ExecutionEngine {
    oms: Arc<OMS>,
    orderbook: Arc<RwLock<OrderBook>>,
}

impl ExecutionEngine {
    pub fn new(oms: Arc<OMS>, orderbook: Arc<RwLock<OrderBook>>) -> Self {
        Self { oms, orderbook }
    }

    pub fn get_oms(&self) -> Arc<OMS> {
        Arc::clone(&self.oms)
    }

    /// Start the execution engine
    /// - Syncs order states periodically
    /// - Fetches and processes fills
    pub async fn start(self, sync_interval_ms: u64) -> Result<()> {
        let mut ticker = interval(Duration::from_millis(sync_interval_ms));

        loop {
            ticker.tick().await;

            // Get current mid price for fill processing
            let book = self.orderbook.read().await;
            let mid_price = book.mid_price().unwrap_or(Decimal::ZERO);
            let symbol = book.symbol.clone();
            drop(book);

            // Sync order states
            if let Err(e) = self.oms.sync_orders(Some(&symbol)).await {
                crate::logging::log_error(
                    "execution_engine",
                    &format!("Failed to sync orders: {}", e),
                );
            }

            // Process fills
            if let Err(e) = self.oms.process_fills(Some(&symbol), mid_price).await {
                crate::logging::log_error(
                    "execution_engine",
                    &format!("Failed to process fills: {}", e),
                );
            }
        }
    }
}
