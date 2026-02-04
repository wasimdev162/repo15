use anyhow::Result;
use dashmap::DashMap;
use rust_decimal::Decimal;
use std::sync::Arc;
use uuid::Uuid;

use crate::exchange::{Exchange, Fill, Order, OrderRequest, OrderStatus, TimeInForce};
use crate::logging;

/// Order state tracked by OMS
#[derive(Debug, Clone)]
pub struct OrderState {
    pub order: Order,
    pub request: OrderRequest,
    pub created_at: i64,
    pub last_update: i64,
}

/// Order Management System
pub struct OMS {
    exchange: Arc<dyn Exchange>,
    orders: Arc<DashMap<String, OrderState>>,  // client_order_id -> OrderState
    fills: Arc<DashMap<String, Vec<Fill>>>,     // order_id -> fills
}

impl OMS {
    pub fn new(exchange: Arc<dyn Exchange>) -> Self {
        Self {
            exchange,
            orders: Arc::new(DashMap::new()),
            fills: Arc::new(DashMap::new()),
        }
    }

    /// Generate a unique client order ID
    pub fn generate_client_order_id(&self, prefix: &str) -> String {
        format!("{}_{}", prefix, Uuid::new_v4().to_string())
    }

    /// Place a new order
    pub async fn place_order(&self, mut request: OrderRequest) -> Result<Order> {
        // Ensure client order ID is set
        if request.client_order_id.is_empty() {
            request.client_order_id = self.generate_client_order_id("order");
        }

        // Log order event
        logging::log_order_event(
            "",  // order_id not known yet
            &request.client_order_id,
            &request.symbol,
            &request.side.to_string(),
            "PLACE",
            request.price.map(|p| p.to_string().parse().unwrap_or(0.0)),
            request.quantity.to_string().parse().unwrap_or(0.0),
            request.time_in_force == TimeInForce::PostOnly,
        );

        // Place order with exchange
        let order = self.exchange.place_order(request.clone()).await?;

        // Store order state
        let state = OrderState {
            order: order.clone(),
            request,
            created_at: chrono::Utc::now().timestamp_millis(),
            last_update: chrono::Utc::now().timestamp_millis(),
        };

        self.orders.insert(order.client_order_id.clone(), state);

        // Log order placed
        logging::log_order_event(
            &order.order_id,
            &order.client_order_id,
            &order.symbol,
            &order.side.to_string(),
            "PLACED",
            order.price.map(|p| p.to_string().parse().unwrap_or(0.0)),
            order.quantity.to_string().parse().unwrap_or(0.0),
            order.time_in_force == TimeInForce::PostOnly,
        );

        Ok(order)
    }

    /// Cancel an order
    pub async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<()> {
        // Find order by order_id
        let client_order_id = self.find_client_order_id(order_id);

        logging::log_order_event(
            order_id,
            &client_order_id.as_ref().map(|s| s.as_str()).unwrap_or_default(),
            symbol,
            "",
            "CANCEL",
            None,
            0.0,
            false,
        );

        self.exchange.cancel_order(symbol, order_id).await?;

        // Update order state
        if let Some(client_id) = &client_order_id {
            if let Some(mut state) = self.orders.get_mut(client_id) {
                state.order.status = OrderStatus::Cancelled;
                state.last_update = chrono::Utc::now().timestamp_millis();
            }
        }

        logging::log_order_event(
            order_id,
            &client_order_id.as_ref().map(|s| s.as_str()).unwrap_or_default(),
            symbol,
            "",
            "CANCELLED",
            None,
            0.0,
            false,
        );

        Ok(())
    }

    /// Replace an order
    pub async fn replace_order(
        &self,
        symbol: &str,
        old_order_id: &str,
        new_request: OrderRequest,
    ) -> Result<Order> {
        logging::log_order_event(
            old_order_id,
            &new_request.client_order_id,
            symbol,
            &new_request.side.to_string(),
            "REPLACE",
            new_request.price.map(|p| p.to_string().parse().unwrap_or(0.0)),
            new_request.quantity.to_string().parse().unwrap_or(0.0),
            new_request.time_in_force == TimeInForce::PostOnly,
        );

        let order = self
            .exchange
            .replace_order(symbol, old_order_id, new_request.clone())
            .await?;

        // Store new order state
        let state = OrderState {
            order: order.clone(),
            request: new_request,
            created_at: chrono::Utc::now().timestamp_millis(),
            last_update: chrono::Utc::now().timestamp_millis(),
        };

        self.orders.insert(order.client_order_id.clone(), state);

        logging::log_order_event(
            &order.order_id,
            &order.client_order_id,
            &order.symbol,
            &order.side.to_string(),
            "REPLACED",
            order.price.map(|p| p.to_string().parse().unwrap_or(0.0)),
            order.quantity.to_string().parse().unwrap_or(0.0),
            order.time_in_force == TimeInForce::PostOnly,
        );

        Ok(order)
    }

    /// Get open orders
    pub async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>> {
        self.exchange.get_open_orders(symbol).await
    }

    /// Sync order states from exchange
    pub async fn sync_orders(&self, symbol: Option<&str>) -> Result<()> {
        let orders = self.exchange.get_open_orders(symbol).await?;

        for order in orders {
            if let Some(mut state) = self.orders.get_mut(&order.client_order_id) {
                state.order = order;
                state.last_update = chrono::Utc::now().timestamp_millis();
            }
        }

        Ok(())
    }

    /// Fetch and process fills
    pub async fn process_fills(&self, symbol: Option<&str>, mid_price: Decimal) -> Result<()> {
        let fills = self.exchange.get_fills(symbol, 50).await?;

        for fill in fills {
            // Check if we've already processed this fill
            let already_processed = self
                .fills
                .get(&fill.order_id)
                .map(|fills| fills.iter().any(|f| f.trade_id == fill.trade_id))
                .unwrap_or(false);

            if already_processed {
                continue;
            }

            // Store fill
            self.fills
                .entry(fill.order_id.clone())
                .or_insert_with(Vec::new)
                .push(fill.clone());

            // Log fill event
            let mid_f64: f64 = mid_price.to_string().parse().unwrap_or(0.0);
            logging::log_fill_event(
                &fill.order_id,
                &fill.client_order_id,
                &fill.symbol,
                &fill.side.to_string(),
                fill.price.to_string().parse().unwrap_or(0.0),
                fill.quantity.to_string().parse().unwrap_or(0.0),
                fill.is_maker,
                mid_f64,
                fill.fee.to_string().parse().unwrap_or(0.0),
            );

            // Update order state if we have it
            if let Some(mut state) = self.orders.get_mut(&fill.client_order_id) {
                // Update filled quantity
                let fill_qty: Decimal = fill.quantity;
                state.order.filled_quantity += fill_qty;
                
                // Update status
                if state.order.filled_quantity >= state.order.quantity {
                    state.order.status = OrderStatus::Filled;
                } else if state.order.filled_quantity > Decimal::ZERO {
                    state.order.status = OrderStatus::PartiallyFilled;
                }
                
                state.last_update = chrono::Utc::now().timestamp_millis();
            }
        }

        Ok(())
    }

    /// Cancel all orders for a symbol
    pub async fn cancel_all_orders(&self, symbol: &str) -> Result<()> {
        logging::log_info("oms", &format!("Cancelling all orders for {}", symbol));
        
        self.exchange.cancel_all_orders(symbol).await?;

        // Update all tracked orders for this symbol
        for mut entry in self.orders.iter_mut() {
            if entry.order.symbol == symbol {
                entry.order.status = OrderStatus::Cancelled;
                entry.last_update = chrono::Utc::now().timestamp_millis();
            }
        }

        Ok(())
    }

    /// Get order state by client order ID
    pub fn get_order_state(&self, client_order_id: &str) -> Option<OrderState> {
        self.orders.get(client_order_id).map(|s| s.clone())
    }

    /// Get all tracked orders
    pub fn get_all_orders(&self) -> Vec<OrderState> {
        self.orders.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Get fills for an order
    pub fn get_order_fills(&self, order_id: &str) -> Vec<Fill> {
        self.fills
            .get(order_id)
            .map(|fills| fills.clone())
            .unwrap_or_default()
    }

    /// Find client order ID by exchange order ID
    fn find_client_order_id(&self, order_id: &str) -> Option<String> {
        self.orders
            .iter()
            .find(|entry| entry.order.order_id == order_id)
            .map(|entry| entry.key().clone())
    }

    /// Get statistics
    pub fn get_stats(&self) -> OMSStats {
        let total_orders = self.orders.len();
        let mut filled = 0;
        let mut partially_filled = 0;
        let mut cancelled = 0;
        let mut active = 0;

        for entry in self.orders.iter() {
            match entry.order.status {
                OrderStatus::Filled => filled += 1,
                OrderStatus::PartiallyFilled => partially_filled += 1,
                OrderStatus::Cancelled => cancelled += 1,
                OrderStatus::New => active += 1,
                _ => {}
            }
        }

        let total_fills = self.fills.iter().map(|entry| entry.len()).sum();

        OMSStats {
            total_orders,
            active_orders: active,
            filled_orders: filled,
            partially_filled_orders: partially_filled,
            cancelled_orders: cancelled,
            total_fills,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OMSStats {
    pub total_orders: usize,
    pub active_orders: usize,
    pub filled_orders: usize,
    pub partially_filled_orders: usize,
    pub cancelled_orders: usize,
    pub total_fills: usize,
}
