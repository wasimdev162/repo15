use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use crate::md::{OrderBook, Trade};

/// Microstructure signals computed from order book and trade data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrostructureSignals {
    /// Order Book Imbalance (OBI)
    pub obi: f64,
    /// Microprice (volume-weighted mid)
    pub microprice: f64,
    /// Spread in basis points
    pub spread_bps: f64,
    /// Spread regime: tight (0), normal (1), wide (2)
    pub spread_regime: u8,
    /// Trade Flow Imbalance (TFI)
    pub tfi: f64,
    /// Short-term volatility (rolling std dev)
    pub volatility: f64,
    /// Quote stuffing score (message rate / trade rate)
    pub stuffing_score: f64,
    /// Iceberg detection score
    pub iceberg_score: f64,
}

impl Default for MicrostructureSignals {
    fn default() -> Self {
        Self {
            obi: 0.0,
            microprice: 0.0,
            spread_bps: 0.0,
            spread_regime: 1,
            tfi: 0.0,
            volatility: 0.0,
            stuffing_score: 0.0,
            iceberg_score: 0.0,
        }
    }
}

/// Signal calculator with state tracking
pub struct SignalCalculator {
    // Price history for volatility
    mid_prices: VecDeque<f64>,
    max_price_history: usize,
    
    // Trade history for TFI
    recent_trades: VecDeque<(i64, f64, f64)>,  // (timestamp, price, signed_qty)
    max_trade_history: usize,
    
    // Order book update tracking for stuffing detection
    book_updates: VecDeque<i64>,  // timestamps
    trades_count: VecDeque<i64>,  // timestamps
    stuffing_window_ms: i64,
    
    // Spread thresholds for regime classification (in bps)
    tight_spread_threshold: f64,
    wide_spread_threshold: f64,
    
    // Iceberg detection state
    same_side_fills: u32,
    last_fill_price: Option<Decimal>,
}

impl SignalCalculator {
    pub fn new() -> Self {
        Self {
            mid_prices: VecDeque::with_capacity(100),
            max_price_history: 100,
            
            recent_trades: VecDeque::with_capacity(50),
            max_trade_history: 50,
            
            book_updates: VecDeque::with_capacity(1000),
            trades_count: VecDeque::with_capacity(1000),
            stuffing_window_ms: 1000,  // 1 second window
            
            tight_spread_threshold: 5.0,  // 5 bps
            wide_spread_threshold: 20.0,  // 20 bps
            
            same_side_fills: 0,
            last_fill_price: None,
        }
    }

    /// Compute all microstructure signals from current market state
    pub fn compute_signals(&mut self, book: &OrderBook, recent_trades: &[Trade]) -> MicrostructureSignals {
        let obi = self.compute_obi(book);
        let microprice = self.compute_microprice(book);
        let spread_bps = book.spread_bps()
            .unwrap_or_default()
            .to_string()
            .parse()
            .unwrap_or(0.0);
        let spread_regime = self.classify_spread_regime(spread_bps);
        let tfi = self.compute_tfi(recent_trades);
        let volatility = self.compute_volatility(book);
        let stuffing_score = self.compute_stuffing_score(book.timestamp);
        let iceberg_score = self.compute_iceberg_score(recent_trades);

        MicrostructureSignals {
            obi,
            microprice,
            spread_bps,
            spread_regime,
            tfi,
            volatility,
            stuffing_score,
            iceberg_score,
        }
    }

    /// Order Book Imbalance (OBI)
    /// OBI = (bid_qty - ask_qty) / (bid_qty + ask_qty)
    /// Range: [-1, 1], positive = more bid pressure
    fn compute_obi(&self, book: &OrderBook) -> f64 {
        let bid_qty = book.depth_qty(crate::md::types::Side::Buy, 5);
        let ask_qty = book.depth_qty(crate::md::types::Side::Sell, 5);
        
        let bid_f64: f64 = bid_qty.to_string().parse().unwrap_or(0.0);
        let ask_f64: f64 = ask_qty.to_string().parse().unwrap_or(0.0);
        
        let total = bid_f64 + ask_f64;
        if total > 0.0 {
            (bid_f64 - ask_f64) / total
        } else {
            0.0
        }
    }

    /// Microprice (volume-weighted mid)
    /// Microprice = (bid_price * ask_qty + ask_price * bid_qty) / (bid_qty + ask_qty)
    fn compute_microprice(&self, book: &OrderBook) -> f64 {
        if let (Some(bid), Some(ask)) = (book.best_bid(), book.best_ask()) {
            let bid_price: f64 = bid.price.to_string().parse().unwrap_or(0.0);
            let ask_price: f64 = ask.price.to_string().parse().unwrap_or(0.0);
            let bid_qty: f64 = bid.quantity.to_string().parse().unwrap_or(0.0);
            let ask_qty: f64 = ask.quantity.to_string().parse().unwrap_or(0.0);
            
            let total_qty = bid_qty + ask_qty;
            if total_qty > 0.0 {
                (bid_price * ask_qty + ask_price * bid_qty) / total_qty
            } else {
                (bid_price + ask_price) / 2.0
            }
        } else {
            0.0
        }
    }

    /// Classify spread regime based on thresholds
    fn classify_spread_regime(&self, spread_bps: f64) -> u8 {
        if spread_bps < self.tight_spread_threshold {
            0  // tight
        } else if spread_bps < self.wide_spread_threshold {
            1  // normal
        } else {
            2  // wide
        }
    }

    /// Trade Flow Imbalance (TFI)
    /// Sum of signed trade quantities over recent window
    /// Positive = more aggressive buying
    fn compute_tfi(&mut self, recent_trades: &[Trade]) -> f64 {
        // Update trade history
        let current_time = chrono::Utc::now().timestamp_millis();
        let window_ms = 5000;  // 5 second window

        for trade in recent_trades {
            let price: f64 = trade.price.to_string().parse().unwrap_or(0.0);
            let qty: f64 = trade.quantity.to_string().parse().unwrap_or(0.0);
            
            // Sign the quantity: positive for buy, negative for sell
            let signed_qty = match trade.side {
                crate::md::types::Side::Buy => qty,
                crate::md::types::Side::Sell => -qty,
            };
            
            self.recent_trades.push_back((trade.timestamp, price, signed_qty));
        }

        // Remove old trades
        while let Some((ts, _, _)) = self.recent_trades.front() {
            if current_time - ts > window_ms {
                self.recent_trades.pop_front();
            } else {
                break;
            }
        }

        // Keep max history
        while self.recent_trades.len() > self.max_trade_history {
            self.recent_trades.pop_front();
        }

        // Sum signed quantities
        let tfi: f64 = self.recent_trades.iter().map(|(_, _, sq)| sq).sum();
        
        // Normalize by number of trades
        if !self.recent_trades.is_empty() {
            tfi / self.recent_trades.len() as f64
        } else {
            0.0
        }
    }

    /// Short-term volatility (rolling standard deviation of mid prices)
    fn compute_volatility(&mut self, book: &OrderBook) -> f64 {
        if let Some(mid) = book.mid_price() {
            let mid_f64: f64 = mid.to_string().parse().unwrap_or(0.0);
            self.mid_prices.push_back(mid_f64);
            
            // Keep max history
            while self.mid_prices.len() > self.max_price_history {
                self.mid_prices.pop_front();
            }
            
            // Need at least 10 prices for meaningful volatility
            if self.mid_prices.len() < 10 {
                return 0.0;
            }
            
            // Compute returns
            let mut returns = Vec::new();
            for i in 1..self.mid_prices.len() {
                let ret = (self.mid_prices[i] - self.mid_prices[i - 1]) / self.mid_prices[i - 1];
                returns.push(ret);
            }
            
            // Compute standard deviation
            let mean: f64 = returns.iter().sum::<f64>() / returns.len() as f64;
            let variance: f64 = returns.iter()
                .map(|r| (r - mean).powi(2))
                .sum::<f64>() / returns.len() as f64;
            
            let std_dev = variance.sqrt();
            
            // Annualized volatility (assuming 1 update per 250ms)
            std_dev * (4.0 * 60.0 * 60.0 * 24.0 * 365.0_f64).sqrt()
        } else {
            0.0
        }
    }

    /// Quote stuffing score
    /// Ratio of order book updates to actual trades
    /// High values indicate potential quote stuffing
    pub fn record_book_update(&mut self, timestamp: i64) {
        self.book_updates.push_back(timestamp);
        
        // Clean old updates
        while let Some(&ts) = self.book_updates.front() {
            if timestamp - ts > self.stuffing_window_ms {
                self.book_updates.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn record_trade(&mut self, timestamp: i64) {
        self.trades_count.push_back(timestamp);
        
        // Clean old trades
        while let Some(&ts) = self.trades_count.front() {
            if timestamp - ts > self.stuffing_window_ms {
                self.trades_count.pop_front();
            } else {
                break;
            }
        }
    }

    fn compute_stuffing_score(&self, _current_time: i64) -> f64 {
        let update_count = self.book_updates.len() as f64;
        let trade_count = self.trades_count.len() as f64;
        
        if trade_count > 0.0 {
            update_count / trade_count
        } else if update_count > 0.0 {
            100.0  // High score if updates but no trades
        } else {
            0.0
        }
    }

    /// Iceberg order detection
    /// Detects repeated fills at the same price level
    fn compute_iceberg_score(&mut self, recent_trades: &[Trade]) -> f64 {
        if recent_trades.is_empty() {
            return 0.0;
        }

        // Check for repeated fills at same price
        for trade in recent_trades {
            if let Some(last_price) = self.last_fill_price {
                if trade.price == last_price {
                    self.same_side_fills += 1;
                } else {
                    self.same_side_fills = 1;
                    self.last_fill_price = Some(trade.price);
                }
            } else {
                self.same_side_fills = 1;
                self.last_fill_price = Some(trade.price);
            }
        }

        // Score based on consecutive fills
        // More consecutive fills = higher probability of iceberg
        if self.same_side_fills >= 5 {
            1.0
        } else if self.same_side_fills >= 3 {
            0.5
        } else {
            0.0
        }
    }
}

impl Default for SignalCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_obi_calculation() {
        let calc = SignalCalculator::new();
        
        let mut book = OrderBook::new("TEST/USDT".to_string());
        book.update_level(dec!(100), dec!(10), crate::md::types::Side::Buy);
        book.update_level(dec!(101), dec!(5), crate::md::types::Side::Sell);
        
        let obi = calc.compute_obi(&book);
        
        // OBI = (10 - 5) / (10 + 5) = 5/15 = 0.333...
        assert!((obi - 0.333).abs() < 0.01);
    }

    #[test]
    fn test_microprice_calculation() {
        let calc = SignalCalculator::new();
        
        let mut book = OrderBook::new("TEST/USDT".to_string());
        book.update_level(dec!(100), dec!(10), crate::md::types::Side::Buy);
        book.update_level(dec!(101), dec!(10), crate::md::types::Side::Sell);
        
        let microprice = calc.compute_microprice(&book);
        
        // Microprice = (100*10 + 101*10) / 20 = 2010/20 = 100.5
        assert!((microprice - 100.5).abs() < 0.01);
    }

    #[test]
    fn test_spread_regime_classification() {
        let calc = SignalCalculator::new();
        
        assert_eq!(calc.classify_spread_regime(3.0), 0);  // tight
        assert_eq!(calc.classify_spread_regime(10.0), 1);  // normal
        assert_eq!(calc.classify_spread_regime(25.0), 2);  // wide
    }
}
