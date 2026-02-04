use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hmac::{Hmac, Mac};
use reqwest::Client;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use super::interface::Exchange;
use super::types::*;

type HmacSha256 = Hmac<Sha256>;

/// Bybit REST API response wrapper
#[derive(Debug, Deserialize)]
struct BybitResponse<T> {
    #[serde(rename = "retCode")]
    ret_code: i32,
    #[serde(rename = "retMsg")]
    ret_msg: String,
    result: Option<T>,
    #[serde(rename = "retExtInfo")]
    ret_ext_info: Option<Value>,
    time: i64,
}

/// Bybit order response
#[derive(Debug, Deserialize)]
struct BybitOrderResult {
    #[serde(rename = "orderId")]
    order_id: String,
    #[serde(rename = "orderLinkId")]
    order_link_id: String,
}

/// Bybit order query result
#[derive(Debug, Deserialize)]
struct BybitOrderInfo {
    #[serde(rename = "orderId")]
    order_id: String,
    #[serde(rename = "orderLinkId")]
    order_link_id: Option<String>,
    symbol: String,
    side: String,
    #[serde(rename = "orderType")]
    order_type: String,
    price: Option<String>,
    qty: String,
    #[serde(rename = "cumExecQty")]
    cum_exec_qty: String,
    #[serde(rename = "avgPrice")]
    avg_price: Option<String>,
    #[serde(rename = "orderStatus")]
    order_status: String,
    #[serde(rename = "timeInForce")]
    time_in_force: String,
    #[serde(rename = "createdTime")]
    created_time: String,
    #[serde(rename = "updatedTime")]
    updated_time: String,
}

/// Bybit order list response
#[derive(Debug, Deserialize)]
struct BybitOrderList {
    list: Vec<BybitOrderInfo>,
}

/// Bybit fill/execution info
#[derive(Debug, Deserialize)]
struct BybitExecution {
    #[serde(rename = "execId")]
    exec_id: String,
    #[serde(rename = "orderId")]
    order_id: String,
    #[serde(rename = "orderLinkId")]
    order_link_id: Option<String>,
    symbol: String,
    side: String,
    #[serde(rename = "execPrice")]
    exec_price: String,
    #[serde(rename = "execQty")]
    exec_qty: String,
    #[serde(rename = "execFee")]
    exec_fee: String,
    #[serde(rename = "feeRate")]
    fee_rate: Option<String>,
    #[serde(rename = "execType")]
    exec_type: String,
    #[serde(rename = "execTime")]
    exec_time: String,
    #[serde(rename = "isMaker")]
    is_maker: bool,
}

/// Bybit execution list response
#[derive(Debug, Deserialize)]
struct BybitExecutionList {
    list: Vec<BybitExecution>,
}

/// Bybit balance info
#[derive(Debug, Deserialize)]
struct BybitBalance {
    coin: String,
    #[serde(rename = "walletBalance")]
    wallet_balance: String,
    #[serde(rename = "availableToWithdraw")]
    available_to_withdraw: String,
}

/// Bybit balance list response
#[derive(Debug, Deserialize)]
struct BybitBalanceList {
    list: Vec<BybitBalanceInfo>,
}

#[derive(Debug, Deserialize)]
struct BybitBalanceInfo {
    coin: Vec<BybitBalance>,
}

pub struct BybitExchange {
    api_key: String,
    api_secret: String,
    base_url: String,
    category: String,
    client: Client,
}

impl BybitExchange {
    pub fn new(
        api_key: String,
        api_secret: String,
        base_url: String,
        category: String,
    ) -> Self {
        Self {
            api_key,
            api_secret,
            base_url,
            category,
            client: Client::new(),
        }
    }

    /// Generate authentication signature
    fn generate_signature(&self, timestamp: u128, params: &str) -> String {
        let recv_window = "5000";
        let sign_str = format!("{}{}{}{}", timestamp, &self.api_key, recv_window, params);

        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(sign_str.as_bytes());

        hex::encode(mac.finalize().into_bytes())
    }

    /// Make authenticated POST request
    async fn post_request<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        params: HashMap<String, Value>,
    ) -> Result<T> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis();

        let params_json = serde_json::to_string(&params)?;
        let signature = self.generate_signature(timestamp, &params_json);

        let url = format!("{}{}", self.base_url, endpoint);

        let response = self
            .client
            .post(&url)
            .header("X-BAPI-API-KEY", &self.api_key)
            .header("X-BAPI-TIMESTAMP", timestamp.to_string())
            .header("X-BAPI-SIGN", signature)
            .header("X-BAPI-RECV-WINDOW", "5000")
            .header("Content-Type", "application/json")
            .body(params_json)
            .send()
            .await
            .context("Failed to send request")?;

        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            return Err(anyhow!("HTTP error {}: {}", status, text));
        }

        let bybit_response: BybitResponse<T> = serde_json::from_str(&text)
            .context(format!("Failed to parse response: {}", text))?;

        if bybit_response.ret_code != 0 {
            return Err(anyhow!(
                "Bybit API error {}: {}",
                bybit_response.ret_code,
                bybit_response.ret_msg
            ));
        }

        bybit_response
            .result
            .ok_or_else(|| anyhow!("Empty result from Bybit"))
    }

    /// Make authenticated GET request
    async fn get_request<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        params: HashMap<String, String>,
    ) -> Result<T> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis();

        let mut query_params: Vec<String> = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        query_params.sort();
        let query_string = query_params.join("&");

        let signature = self.generate_signature(timestamp, &query_string);

        let url = format!("{}{}?{}", self.base_url, endpoint, query_string);

        let response = self
            .client
            .get(&url)
            .header("X-BAPI-API-KEY", &self.api_key)
            .header("X-BAPI-TIMESTAMP", timestamp.to_string())
            .header("X-BAPI-SIGN", signature)
            .header("X-BAPI-RECV-WINDOW", "5000")
            .send()
            .await
            .context("Failed to send request")?;

        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            return Err(anyhow!("HTTP error {}: {}", status, text));
        }

        let bybit_response: BybitResponse<T> = serde_json::from_str(&text)
            .context(format!("Failed to parse response: {}", text))?;

        if bybit_response.ret_code != 0 {
            return Err(anyhow!(
                "Bybit API error {}: {}",
                bybit_response.ret_code,
                bybit_response.ret_msg
            ));
        }

        bybit_response
            .result
            .ok_or_else(|| anyhow!("Empty result from Bybit"))
    }

    /// Convert internal order to Bybit order info
    fn parse_order_info(&self, info: BybitOrderInfo) -> Result<Order> {
        Ok(Order {
            order_id: info.order_id,
            client_order_id: info.order_link_id.unwrap_or_default(),
            symbol: info.symbol,
            side: match info.side.as_str() {
                "Buy" => OrderSide::Buy,
                _ => OrderSide::Sell,
            },
            order_type: match info.order_type.as_str() {
                "Market" => OrderType::Market,
                _ => OrderType::Limit,
            },
            quantity: info.qty.parse()?,
            price: info.price.and_then(|p| p.parse().ok()),
            filled_quantity: info.cum_exec_qty.parse()?,
            average_price: info.avg_price.and_then(|p| p.parse().ok()),
            status: match info.order_status.as_str() {
                "New" => OrderStatus::New,
                "PartiallyFilled" => OrderStatus::PartiallyFilled,
                "Filled" => OrderStatus::Filled,
                "Cancelled" => OrderStatus::Cancelled,
                "Rejected" => OrderStatus::Rejected,
                _ => OrderStatus::Expired,
            },
            time_in_force: match info.time_in_force.as_str() {
                "GTC" => TimeInForce::GTC,
                "IOC" => TimeInForce::IOC,
                "FOK" => TimeInForce::FOK,
                _ => TimeInForce::PostOnly,
            },
            created_at: info.created_time.parse()?,
            updated_at: info.updated_time.parse()?,
        })
    }

    /// Parse execution into fill
    fn parse_execution(&self, exec: BybitExecution) -> Result<Fill> {
        Ok(Fill {
            trade_id: exec.exec_id,
            order_id: exec.order_id,
            client_order_id: exec.order_link_id.unwrap_or_default(),
            symbol: exec.symbol,
            side: match exec.side.as_str() {
                "Buy" => OrderSide::Buy,
                _ => OrderSide::Sell,
            },
            price: exec.exec_price.parse()?,
            quantity: exec.exec_qty.parse()?,
            fee: exec.exec_fee.parse()?,
            fee_currency: "USDT".to_string(),  // Assuming USDT
            is_maker: exec.is_maker,
            timestamp: exec.exec_time.parse()?,
        })
    }
}

#[async_trait]
impl Exchange for BybitExchange {
    async fn place_order(&self, request: OrderRequest) -> Result<Order> {
        let mut params = HashMap::new();
        params.insert("category".to_string(), Value::String(self.category.clone()));
        params.insert("symbol".to_string(), Value::String(request.symbol.clone()));
        params.insert("side".to_string(), Value::String(request.side.to_string()));
        params.insert("orderType".to_string(), Value::String(request.order_type.to_string()));
        params.insert("qty".to_string(), Value::String(request.quantity.to_string()));
        params.insert("timeInForce".to_string(), Value::String(request.time_in_force.to_string()));
        params.insert("orderLinkId".to_string(), Value::String(request.client_order_id.clone()));

        if let Some(price) = request.price {
            params.insert("price".to_string(), Value::String(price.to_string()));
        }

        let result: BybitOrderResult = self.post_request("/v5/order/create", params).await?;

        // Fetch the order details
        self.get_order(&request.symbol, &result.order_id).await
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<()> {
        let mut params = HashMap::new();
        params.insert("category".to_string(), Value::String(self.category.clone()));
        params.insert("symbol".to_string(), Value::String(symbol.to_string()));
        params.insert("orderId".to_string(), Value::String(order_id.to_string()));

        let _: BybitOrderResult = self.post_request("/v5/order/cancel", params).await?;
        Ok(())
    }

    async fn replace_order(
        &self,
        symbol: &str,
        old_order_id: &str,
        new_request: OrderRequest,
    ) -> Result<Order> {
        // Cancel old order
        self.cancel_order(symbol, old_order_id).await?;

        // Place new order
        self.place_order(new_request).await
    }

    async fn get_open_orders(&self, symbol: Option<&str>) -> Result<Vec<Order>> {
        let mut params = HashMap::new();
        params.insert("category".to_string(), self.category.clone());
        
        if let Some(sym) = symbol {
            params.insert("symbol".to_string(), sym.to_string());
        }

        let result: BybitOrderList = self.get_request("/v5/order/realtime", params).await?;

        result
            .list
            .into_iter()
            .map(|info| self.parse_order_info(info))
            .collect()
    }

    async fn get_order(&self, symbol: &str, order_id: &str) -> Result<Order> {
        let mut params = HashMap::new();
        params.insert("category".to_string(), self.category.clone());
        params.insert("symbol".to_string(), symbol.to_string());
        params.insert("orderId".to_string(), order_id.to_string());

        let result: BybitOrderList = self.get_request("/v5/order/realtime", params).await?;

        result
            .list
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("Order not found"))
            .and_then(|info| self.parse_order_info(info))
    }

    async fn get_fills(&self, symbol: Option<&str>, limit: usize) -> Result<Vec<Fill>> {
        let mut params = HashMap::new();
        params.insert("category".to_string(), self.category.clone());
        params.insert("limit".to_string(), limit.to_string());
        
        if let Some(sym) = symbol {
            params.insert("symbol".to_string(), sym.to_string());
        }

        let result: BybitExecutionList = self.get_request("/v5/execution/list", params).await?;

        result
            .list
            .into_iter()
            .map(|exec| self.parse_execution(exec))
            .collect()
    }

    async fn get_balances(&self) -> Result<Vec<Balance>> {
        let mut params = HashMap::new();
        params.insert("accountType".to_string(), "UNIFIED".to_string());

        let result: BybitBalanceList = self.get_request("/v5/account/wallet-balance", params).await?;

        let mut balances = Vec::new();
        for info in result.list {
            for coin in info.coin {
                let total: Decimal = coin.wallet_balance.parse()?;
                let available: Decimal = coin.available_to_withdraw.parse()?;
                balances.push(Balance {
                    currency: coin.coin,
                    available,
                    locked: total - available,
                    total,
                });
            }
        }

        Ok(balances)
    }

    async fn cancel_all_orders(&self, symbol: &str) -> Result<()> {
        let mut params = HashMap::new();
        params.insert("category".to_string(), Value::String(self.category.clone()));
        params.insert("symbol".to_string(), Value::String(symbol.to_string()));

        let _: Value = self.post_request("/v5/order/cancel-all", params).await?;
        Ok(())
    }
}
