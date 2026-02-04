use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tracing::info;

/// Event types for structured logging
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventType {
    /// System startup
    SystemStart,
    /// System shutdown
    SystemShutdown,
    /// Market data snapshot
    MarketSnapshot,
    /// Trading decision
    Decision,
    /// Order event (place/cancel/replace)
    OrderEvent,
    /// Fill event
    FillEvent,
    /// Performance metrics update
    MetricsUpdate,
    /// Error event
    Error,
    /// Warning event
    Warning,
    /// Info event
    Info,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventType::SystemStart => write!(f, "SYSTEM_START"),
            EventType::SystemShutdown => write!(f, "SYSTEM_SHUTDOWN"),
            EventType::MarketSnapshot => write!(f, "MARKET_SNAPSHOT"),
            EventType::Decision => write!(f, "DECISION"),
            EventType::OrderEvent => write!(f, "ORDER_EVENT"),
            EventType::FillEvent => write!(f, "FILL_EVENT"),
            EventType::MetricsUpdate => write!(f, "METRICS_UPDATE"),
            EventType::Error => write!(f, "ERROR"),
            EventType::Warning => write!(f, "WARNING"),
            EventType::Info => write!(f, "INFO"),
        }
    }
}

/// Structured log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub event_type: EventType,
    pub message: String,
    #[serde(flatten)]
    pub data: HashMap<String, Value>,
}

impl LogEntry {
    pub fn new(event_type: EventType, message: String) -> Self {
        Self {
            timestamp: Utc::now().to_rfc3339(),
            event_type,
            message,
            data: HashMap::new(),
        }
    }

    pub fn with_data(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.data.insert(key.into(), json_value);
        }
        self
    }

    pub fn log(self) {
        if let Ok(json) = serde_json::to_string(&self) {
            info!("{}", json);
        }
    }
}

/// Initialize logging system
pub fn init_logging(log_level: &str) -> anyhow::Result<()> {
    use tracing_subscriber::{fmt, EnvFilter};

    // Parse log level
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    // Initialize subscriber with JSON formatting
    fmt()
        .json()
        .with_env_filter(env_filter)
        .with_current_span(false)
        .with_target(false)
        .init();

    Ok(())
}

/// Log market snapshot
pub fn log_market_snapshot(
    symbol: &str,
    bid: f64,
    ask: f64,
    mid: f64,
    spread_bps: f64,
    obi: Option<f64>,
    microprice: Option<f64>,
    volatility: Option<f64>,
    stuffing_score: Option<f64>,
) {
    LogEntry::new(
        EventType::MarketSnapshot,
        format!("Market snapshot for {}", symbol),
    )
    .with_data("symbol", symbol)
    .with_data("bid", bid)
    .with_data("ask", ask)
    .with_data("mid", mid)
    .with_data("spread_bps", spread_bps)
    .with_data("obi", obi)
    .with_data("microprice", microprice)
    .with_data("volatility", volatility)
    .with_data("stuffing_score", stuffing_score)
    .log();
}

/// Log trading decision
pub fn log_decision(
    symbol: &str,
    action: &str,
    rationale: &str,
    signals: HashMap<String, Value>,
) {
    LogEntry::new(
        EventType::Decision,
        format!("Decision: {} for {}", action, symbol),
    )
    .with_data("symbol", symbol)
    .with_data("action", action)
    .with_data("rationale", rationale)
    .with_data("signals", signals)
    .log();
}

/// Log order event
pub fn log_order_event(
    order_id: &str,
    client_order_id: &str,
    symbol: &str,
    side: &str,
    action: &str,
    price: Option<f64>,
    qty: f64,
    post_only: bool,
) {
    LogEntry::new(
        EventType::OrderEvent,
        format!("Order {}: {} {} {}", action, side, qty, symbol),
    )
    .with_data("order_id", order_id)
    .with_data("client_order_id", client_order_id)
    .with_data("symbol", symbol)
    .with_data("side", side)
    .with_data("action", action)
    .with_data("price", price)
    .with_data("qty", qty)
    .with_data("post_only", post_only)
    .log();
}

/// Log fill event
pub fn log_fill_event(
    order_id: &str,
    client_order_id: &str,
    symbol: &str,
    side: &str,
    fill_price: f64,
    fill_qty: f64,
    is_maker: bool,
    mid_at_fill: f64,
    fee: f64,
) {
    LogEntry::new(
        EventType::FillEvent,
        format!("Fill: {} {} {} @ {}", side, fill_qty, symbol, fill_price),
    )
    .with_data("order_id", order_id)
    .with_data("client_order_id", client_order_id)
    .with_data("symbol", symbol)
    .with_data("side", side)
    .with_data("fill_price", fill_price)
    .with_data("fill_qty", fill_qty)
    .with_data("is_maker", is_maker)
    .with_data("mid_at_fill", mid_at_fill)
    .with_data("fee", fee)
    .log();
}

/// Log metrics update
pub fn log_metrics(
    is_bps: f64,
    vwap_out_bps: f64,
    fill_rate: f64,
    as_1s_bps: Option<f64>,
    as_5s_bps: Option<f64>,
    as_30s_bps: Option<f64>,
    maker_ratio: f64,
    total_filled_qty: f64,
    total_fees: f64,
) {
    LogEntry::new(EventType::MetricsUpdate, "Performance metrics update".to_string())
        .with_data("is_bps", is_bps)
        .with_data("vwap_out_bps", vwap_out_bps)
        .with_data("fill_rate", fill_rate)
        .with_data("as_1s_bps", as_1s_bps)
        .with_data("as_5s_bps", as_5s_bps)
        .with_data("as_30s_bps", as_30s_bps)
        .with_data("maker_ratio", maker_ratio)
        .with_data("total_filled_qty", total_filled_qty)
        .with_data("total_fees", total_fees)
        .log();
}

/// Log system start
pub fn log_system_start(config: &crate::config::Config) {
    LogEntry::new(
        EventType::SystemStart,
        "Algorithmic trading system starting".to_string(),
    )
    .with_data("mode", format!("{:?}", config.mode))
    .with_data("instrument", format!("{}", config.instrument))
    .with_data("symbols", &config.symbols)
    .with_data("run_seconds", config.run_seconds)
    .with_data("testnet", config.bybit_testnet)
    .log();
}

/// Log system shutdown
pub fn log_system_shutdown(reason: &str, graceful: bool) {
    LogEntry::new(
        EventType::SystemShutdown,
        format!("System shutdown: {}", reason),
    )
    .with_data("reason", reason)
    .with_data("graceful", graceful)
    .log();
}

/// Log error
pub fn log_error(context: &str, error: &str) {
    LogEntry::new(EventType::Error, format!("Error in {}: {}", context, error))
        .with_data("context", context)
        .with_data("error", error)
        .log();
}

/// Log warning
pub fn log_warning(context: &str, message: &str) {
    LogEntry::new(
        EventType::Warning,
        format!("Warning in {}: {}", context, message),
    )
    .with_data("context", context)
    .with_data("warning", message)
    .log();
}

/// Log info
pub fn log_info(context: &str, message: &str) {
    LogEntry::new(EventType::Info, format!("{}: {}", context, message))
        .with_data("context", context)
        .with_data("message", message)
        .log();
}
