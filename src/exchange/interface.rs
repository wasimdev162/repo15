use anyhow::Result;
use async_trait::async_trait;
use rust_decimal::Decimal;

use super::types::{Balance, Fill, Order, OrderRequest, OrderSide};

/// Exchange interface trait
#[async_trait]
pub trait Exchange: Send + Sync {
    /// Place a new order
    async fn place_order(&self, request: OrderRequest) -> Result<Order>;

    /// Cancel an existing order
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<()>;

    /// Replace an existing order (cancel and place new)
    async fn replace_order(
        &self,
        symbol: &str,
        old_order_id: &str,
        new_request: OrderRequest,
    ) -> Result<Order>;

    /// Get open orders
    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>>;

    /// Get order by ID
    async fn get_order(&self, symbol: &str, order_id: &str) -> Result<Order>;

    /// Get recent fills
    async fn get_fills(&self, symbol: Option<&str>, limit: usize) -> Result<Vec<Fill>>;

    /// Get account balances
    async fn get_balances(&self) -> Result<Vec<Balance>>;

    /// Cancel all orders for a symbol
    async fn cancel_all_orders(&self, symbol: &str) -> Result<()>;
}
