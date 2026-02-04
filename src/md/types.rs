use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Order book side (bids or asks)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Side::Buy => write!(f, "buy"),
            Side::Sell => write!(f, "sell"),
        }
    }
}

/// A price level in the order book
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level {
    pub price: Decimal,
    pub quantity: Decimal,
}

impl Level {
    pub fn new(price: Decimal, quantity: Decimal) -> Self {
        Self { price, quantity }
    }
}

/// Trade information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub timestamp: i64,
    pub price: Decimal,
    pub quantity: Decimal,
    pub side: Side,
}

/// Order book snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    pub timestamp: i64,
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
}

impl OrderBook {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            timestamp: 0,
            bids: Vec::new(),
            asks: Vec::new(),
        }
    }

    /// Get best bid price and quantity
    pub fn best_bid(&self) -> Option<&Level> {
        self.bids.first()
    }

    /// Get best ask price and quantity
    pub fn best_ask(&self) -> Option<&Level> {
        self.asks.first()
    }

    /// Get mid price
    pub fn mid_price(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some((bid.price + ask.price) / Decimal::from(2)),
            _ => None,
        }
    }

    /// Get spread in basis points
    pub fn spread_bps(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => {
                let spread = ask.price - bid.price;
                let mid = (bid.price + ask.price) / Decimal::from(2);
                if mid > Decimal::ZERO {
                    Some(spread / mid * Decimal::from(10000))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Update the order book with a new level
    pub fn update_level(&mut self, price: Decimal, quantity: Decimal, side: Side) {
        let levels = match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        };

        // Remove level if quantity is zero
        if quantity == Decimal::ZERO {
            levels.retain(|l| l.price != price);
            return;
        }

        // Find and update existing level or insert new one
        match levels.iter_mut().find(|l| l.price == price) {
            Some(level) => level.quantity = quantity,
            None => {
                levels.push(Level::new(price, quantity));
                // Sort: bids descending, asks ascending
                match side {
                    Side::Buy => levels.sort_by(|a, b| b.price.cmp(&a.price)),
                    Side::Sell => levels.sort_by(|a, b| a.price.cmp(&b.price)),
                }
            }
        }
    }

    /// Get total quantity at top N levels on a side
    pub fn depth_qty(&self, side: Side, levels: usize) -> Decimal {
        let book_levels = match side {
            Side::Buy => &self.bids,
            Side::Sell => &self.asks,
        };
        
        book_levels
            .iter()
            .take(levels)
            .map(|l| l.quantity)
            .sum()
    }
}
