use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::types::{OrderBook, Side, Trade};
use crate::logging;

/// Bybit WebSocket message types
#[derive(Debug, Deserialize)]
struct BybitMessage {
    topic: Option<String>,
    #[serde(rename = "type")]
    msg_type: Option<String>,
    ts: Option<i64>,
    data: Option<Value>,
}

/// Order book data from Bybit
#[derive(Debug, Deserialize)]
struct OrderBookData {
    s: String,  // symbol
    b: Vec<Vec<String>>,  // bids [[price, qty], ...]
    a: Vec<Vec<String>>,  // asks [[price, qty], ...]
    u: i64,  // update id
    seq: Option<i64>,
}

/// Trade data from Bybit
#[derive(Debug, Deserialize)]
struct TradeData {
    #[serde(rename = "T")]
    timestamp: i64,
    s: String,  // symbol
    #[serde(rename = "S")]
    side: String,
    v: String,  // volume
    p: String,  // price
    #[serde(rename = "L")]
    tick_direction: Option<String>,
    i: String,  // trade id
}

/// Subscription request
#[derive(Debug, Serialize)]
struct SubscribeRequest {
    op: String,
    args: Vec<String>,
}

pub struct BybitWebSocket {
    url: String,
    symbol: String,
    orderbook: Arc<RwLock<OrderBook>>,
    trades: Arc<RwLock<Vec<Trade>>>,
    max_trades: usize,
}

impl BybitWebSocket {
    pub fn new(url: String, symbol: String) -> Self {
        let orderbook = Arc::new(RwLock::new(OrderBook::new(symbol.clone())));
        let trades = Arc::new(RwLock::new(Vec::with_capacity(1000)));
        
        Self {
            url,
            symbol,
            orderbook,
            trades,
            max_trades: 1000,
        }
    }

    pub fn get_orderbook(&self) -> Arc<RwLock<OrderBook>> {
        Arc::clone(&self.orderbook)
    }

    pub fn get_trades(&self) -> Arc<RwLock<Vec<Trade>>> {
        Arc::clone(&self.trades)
    }

    pub async fn connect_and_subscribe(&mut self) -> Result<()> {
        loop {
            match self.run_connection().await {
                Ok(_) => {
                    logging::log_warning("bybit_ws", "Connection closed normally, reconnecting...");
                }
                Err(e) => {
                    logging::log_error("bybit_ws", &format!("Connection error: {}, reconnecting...", e));
                }
            }
            
            // Wait before reconnecting
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }

    async fn run_connection(&mut self) -> Result<()> {
        logging::log_info("bybit_ws", &format!("Connecting to {}", self.url));

        let (ws_stream, _) = connect_async(&self.url)
            .await
            .context("Failed to connect to WebSocket")?;

        logging::log_info("bybit_ws", "WebSocket connected");

        let (mut write, mut read) = ws_stream.split();

        // Subscribe to orderbook and trades
        let bybit_symbol = self.symbol.replace('/', "");
        let subscribe = SubscribeRequest {
            op: "subscribe".to_string(),
            args: vec![
                format!("orderbook.50.{}", bybit_symbol),
                format!("publicTrade.{}", bybit_symbol),
            ],
        };

        let subscribe_msg = serde_json::to_string(&subscribe)?;
        write
            .send(Message::Text(subscribe_msg))
            .await
            .context("Failed to send subscription")?;

        logging::log_info(
            "bybit_ws",
            &format!("Subscribed to orderbook and trades for {}", bybit_symbol),
        );

        // Process incoming messages
        loop {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            if let Err(e) = self.handle_message(&text).await {
                                logging::log_error("bybit_ws", &format!("Error handling message: {}", e));
                            }
                        }
                        Some(Ok(Message::Ping(_))) => {
                            // Handled automatically by tokio-tungstenite
                        }
                        Some(Ok(Message::Pong(_))) => {
                            // Ignore
                        }
                        Some(Ok(Message::Close(_))) => {
                            logging::log_warning("bybit_ws", "Received close frame");
                            break;
                        }
                        Some(Err(e)) => {
                            logging::log_error("bybit_ws", &format!("WebSocket error: {}", e));
                            break;
                        }
                        None => {
                            logging::log_warning("bybit_ws", "WebSocket stream ended");
                            break;
                        }
                        _ => {}
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(20)) => {
                    // Send ping periodically
                    if write.send(Message::Ping(vec![])).await.is_err() {
                        logging::log_error("bybit_ws", "Failed to send ping");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_message(&self, text: &str) -> Result<()> {
        let msg: BybitMessage = serde_json::from_str(text)
            .context("Failed to parse WebSocket message")?;

        // Check for subscription confirmation
        if let Some(msg_type) = &msg.msg_type {
            if msg_type == "snapshot" || msg_type == "delta" {
                if let Some(topic) = &msg.topic {
                    if topic.starts_with("orderbook") {
                        self.handle_orderbook_message(&msg).await?;
                    } else if topic.starts_with("publicTrade") {
                        self.handle_trade_message(&msg).await?;
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_orderbook_message(&self, msg: &BybitMessage) -> Result<()> {
        if let Some(data) = &msg.data {
            let ob_data: OrderBookData = serde_json::from_value(data.clone())
                .context("Failed to parse orderbook data")?;

            let mut book = self.orderbook.write().await;
            
            // Handle snapshot vs delta
            if let Some(msg_type) = &msg.msg_type {
                if msg_type == "snapshot" {
                    // Clear the book for snapshot
                    book.bids.clear();
                    book.asks.clear();
                }
            }

            // Update timestamp
            book.timestamp = msg.ts.unwrap_or(chrono::Utc::now().timestamp_millis());

            // Update bids
            for bid in &ob_data.b {
                if bid.len() >= 2 {
                    let price: Decimal = bid[0].parse().unwrap_or_default();
                    let qty: Decimal = bid[1].parse().unwrap_or_default();
                    book.update_level(price, qty, Side::Buy);
                }
            }

            // Update asks
            for ask in &ob_data.a {
                if ask.len() >= 2 {
                    let price: Decimal = ask[0].parse().unwrap_or_default();
                    let qty: Decimal = ask[1].parse().unwrap_or_default();
                    book.update_level(price, qty, Side::Sell);
                }
            }
        }

        Ok(())
    }

    async fn handle_trade_message(&self, msg: &BybitMessage) -> Result<()> {
        if let Some(data) = &msg.data {
            // Bybit sends trades as an array
            let trades_data: Vec<TradeData> = serde_json::from_value(data.clone())
                .context("Failed to parse trade data")?;

            let mut trades = self.trades.write().await;

            for trade_data in trades_data {
                let price: Decimal = trade_data.p.parse().unwrap_or_default();
                let quantity: Decimal = trade_data.v.parse().unwrap_or_default();
                let side = match trade_data.side.as_str() {
                    "Buy" => Side::Buy,
                    _ => Side::Sell,
                };

                let trade = Trade {
                    timestamp: trade_data.timestamp,
                    price,
                    quantity,
                    side,
                };

                trades.push(trade);

                // Maintain ring buffer size
                if trades.len() > self.max_trades {
                    trades.remove(0);
                }
            }
        }

        Ok(())
    }
}

/// Market data snapshot logger
pub async fn start_snapshot_logger(
    orderbook: Arc<RwLock<OrderBook>>,
    symbol: String,
    interval_ms: u64,
) {
    let mut ticker = interval(Duration::from_millis(interval_ms));

    loop {
        ticker.tick().await;

        let book = orderbook.read().await;
        
        if let (Some(bid), Some(ask)) = (book.best_bid(), book.best_ask()) {
            let mid = book.mid_price().unwrap_or_default();
            let spread_bps = book.spread_bps().unwrap_or_default();

            logging::log_market_snapshot(
                &symbol,
                bid.price.to_string().parse().unwrap_or(0.0),
                ask.price.to_string().parse().unwrap_or(0.0),
                mid.to_string().parse().unwrap_or(0.0),
                spread_bps.to_string().parse().unwrap_or(0.0),
                None,  // OBI - will be computed by signals module
                None,  // Microprice - will be computed by signals module
                None,  // Volatility - will be computed by signals module
                None,  // Stuffing score - will be computed by signals module
            );
        }
    }
}
