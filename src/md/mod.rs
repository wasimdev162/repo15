pub mod bybit_ws;
pub mod replay;
pub mod types;

pub use bybit_ws::{start_snapshot_logger, BybitWebSocket};
pub use replay::ReplayHandler;
pub use types::{OrderBook, Trade};

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::{Config, Mode};

/// Market data handler that abstracts live vs replay mode
pub struct MarketDataHandler {
    pub orderbook: Arc<RwLock<OrderBook>>,
    pub trades: Arc<RwLock<Vec<Trade>>>,
}

impl MarketDataHandler {
    /// Create and start a new market data handler based on config
    pub async fn start(config: &Config) -> Result<Self> {
        let symbol = config.symbols.first()
            .ok_or_else(|| anyhow::anyhow!("No symbols configured"))?
            .clone();

        match config.mode {
            Mode::Live => {
                crate::logging::log_info("market_data", "Starting live market data feed");
                
                let mut ws = BybitWebSocket::new(
                    config.bybit_ws_public_url.clone(),
                    symbol.clone(),
                );

                let orderbook = ws.get_orderbook();
                let trades = ws.get_trades();

                // Start WebSocket connection in background
                tokio::spawn(async move {
                    if let Err(e) = ws.connect_and_subscribe().await {
                        crate::logging::log_error(
                            "market_data",
                            &format!("WebSocket error: {}", e),
                        );
                    }
                });

                // Start snapshot logger
                let orderbook_clone = Arc::clone(&orderbook);
                let symbol_clone = symbol.clone();
                tokio::spawn(async move {
                    start_snapshot_logger(orderbook_clone, symbol_clone, 250).await;
                });

                Ok(Self { orderbook, trades })
            }
            Mode::Replay => {
                crate::logging::log_info("market_data", "Starting replay mode");
                
                let replay_file = config.replay_file.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Replay file not specified"))?;

                let replay = ReplayHandler::new(replay_file.clone(), symbol.clone());
                let orderbook = replay.get_orderbook();
                let trades = replay.get_trades();

                // Start replay in background
                tokio::spawn(async move {
                    if let Err(e) = replay.start_replay().await {
                        crate::logging::log_error("market_data", &format!("Replay error: {}", e));
                    }
                });

                // Start snapshot logger
                let orderbook_clone = Arc::clone(&orderbook);
                let symbol_clone = symbol.clone();
                tokio::spawn(async move {
                    start_snapshot_logger(orderbook_clone, symbol_clone, 250).await;
                });

                Ok(Self { orderbook, trades })
            }
        }
    }
}
