use anyhow::{Context, Result};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use super::types::{OrderBook, Side, Trade};
use crate::logging;

/// Replay event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReplayEvent {
    OrderBookSnapshot {
        timestamp: i64,
        symbol: String,
        bids: Vec<(String, String)>,  // (price, qty)
        asks: Vec<(String, String)>,
    },
    Trade {
        timestamp: i64,
        symbol: String,
        price: String,
        quantity: String,
        side: String,
    },
}

pub struct ReplayHandler {
    file_path: String,
    symbol: String,
    orderbook: Arc<RwLock<OrderBook>>,
    trades: Arc<RwLock<Vec<Trade>>>,
    speed_multiplier: f64,
}

impl ReplayHandler {
    pub fn new(file_path: String, symbol: String) -> Self {
        let orderbook = Arc::new(RwLock::new(OrderBook::new(symbol.clone())));
        let trades = Arc::new(RwLock::new(Vec::with_capacity(1000)));
        
        Self {
            file_path,
            symbol,
            orderbook,
            trades,
            speed_multiplier: 1.0,
        }
    }

    pub fn get_orderbook(&self) -> Arc<RwLock<OrderBook>> {
        Arc::clone(&self.orderbook)
    }

    pub fn get_trades(&self) -> Arc<RwLock<Vec<Trade>>> {
        Arc::clone(&self.trades)
    }

    pub fn set_speed_multiplier(&mut self, multiplier: f64) {
        self.speed_multiplier = multiplier;
    }

    pub async fn start_replay(&self) -> Result<()> {
        logging::log_info(
            "replay",
            &format!("Starting replay from file: {}", self.file_path),
        );

        let file = File::open(&self.file_path)
            .context(format!("Failed to open replay file: {}", self.file_path))?;

        let reader = BufReader::new(file);
        let mut last_timestamp: Option<i64> = None;

        for (line_num, line) in reader.lines().enumerate() {
            let line = line.context("Failed to read line from replay file")?;
            
            if line.trim().is_empty() {
                continue;
            }

            let event: ReplayEvent = serde_json::from_str(&line)
                .context(format!("Failed to parse line {}: {}", line_num + 1, line))?;

            // Handle timing between events
            let current_timestamp = match &event {
                ReplayEvent::OrderBookSnapshot { timestamp, .. } => *timestamp,
                ReplayEvent::Trade { timestamp, .. } => *timestamp,
            };

            if let Some(last_ts) = last_timestamp {
                let delay_ms = (current_timestamp - last_ts) as f64 / self.speed_multiplier;
                if delay_ms > 0.0 {
                    sleep(Duration::from_millis(delay_ms as u64)).await;
                }
            }

            last_timestamp = Some(current_timestamp);

            // Process event
            match event {
                ReplayEvent::OrderBookSnapshot {
                    timestamp,
                    symbol: _,
                    bids,
                    asks,
                } => {
                    let mut book = self.orderbook.write().await;
                    book.timestamp = timestamp;
                    book.bids.clear();
                    book.asks.clear();

                    for (price_str, qty_str) in bids {
                        let price: Decimal = price_str.parse().unwrap_or_default();
                        let qty: Decimal = qty_str.parse().unwrap_or_default();
                        book.update_level(price, qty, Side::Buy);
                    }

                    for (price_str, qty_str) in asks {
                        let price: Decimal = price_str.parse().unwrap_or_default();
                        let qty: Decimal = qty_str.parse().unwrap_or_default();
                        book.update_level(price, qty, Side::Sell);
                    }
                }
                ReplayEvent::Trade {
                    timestamp,
                    symbol: _,
                    price,
                    quantity,
                    side,
                } => {
                    let price_dec: Decimal = price.parse().unwrap_or_default();
                    let qty_dec: Decimal = quantity.parse().unwrap_or_default();
                    let side_enum = match side.as_str() {
                        "buy" | "Buy" => Side::Buy,
                        _ => Side::Sell,
                    };

                    let trade = Trade {
                        timestamp,
                        price: price_dec,
                        quantity: qty_dec,
                        side: side_enum,
                    };

                    let mut trades = self.trades.write().await;
                    trades.push(trade);
                    if trades.len() > 1000 {
                        trades.remove(0);
                    }
                }
            }
        }

        logging::log_info("replay", "Replay completed");
        Ok(())
    }
}

/// Generate sample replay data for testing
pub fn generate_sample_replay_file(path: &str, symbol: &str) -> Result<()> {
    use std::io::Write;
    
    let mut file = File::create(path)?;
    let base_time = chrono::Utc::now().timestamp_millis();
    let rng = || -> f64 { 1.0 + (rand::random::<f64>() - 0.5) * 0.01 };

    // Generate sample data
    for i in 0..100 {
        let timestamp = base_time + (i * 100);
        
        // Order book snapshot every 10 events
        if i % 10 == 0 {
            let mid_price = 1.0;
            let mut bids = Vec::new();
            let mut asks = Vec::new();

            for j in 0..5 {
                let bid_price = mid_price - 0.0001 * (j + 1) as f64;
                let ask_price = mid_price + 0.0001 * (j + 1) as f64;
                bids.push((format!("{:.4}", bid_price), format!("{:.2}", 100.0 * rng())));
                asks.push((format!("{:.4}", ask_price), format!("{:.2}", 100.0 * rng())));
            }

            let event = ReplayEvent::OrderBookSnapshot {
                timestamp,
                symbol: symbol.to_string(),
                bids,
                asks,
            };

            writeln!(file, "{}", serde_json::to_string(&event)?)?;
        }

        // Trade
        let trade_price = 1.0 + (rand::random::<f64>() - 0.5) * 0.0001;
        let trade_qty = 10.0 * rng();
        let trade_side = if rand::random::<bool>() { "buy" } else { "sell" };

        let event = ReplayEvent::Trade {
            timestamp,
            symbol: symbol.to_string(),
            price: format!("{:.4}", trade_price),
            quantity: format!("{:.2}", trade_qty),
            side: trade_side.to_string(),
        };

        writeln!(file, "{}", serde_json::to_string(&event)?)?;
    }

    Ok(())
}
