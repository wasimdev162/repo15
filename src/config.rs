use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum Mode {
    /// Live trading mode with real market data
    Live,
    /// Replay mode using historical data
    Replay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
pub enum InstrumentType {
    /// Spot trading
    Spot,
    /// Perpetual contracts
    Perp,
    /// Options
    Option,
}

impl std::fmt::Display for InstrumentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InstrumentType::Spot => write!(f, "spot"),
            InstrumentType::Perp => write!(f, "perp"),
            InstrumentType::Option => write!(f, "option"),
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "algorithmic-trading-system")]
#[command(about = "High-frequency algorithmic trading system for Bybit", long_about = None)]
pub struct Cli {
    /// Trading mode: live or replay
    #[arg(long, default_value = "live")]
    pub mode: Mode,

    /// Instrument type: spot, perp, or option
    #[arg(long, default_value = "spot")]
    pub instrument: InstrumentType,

    /// How many seconds to run before stopping
    #[arg(long, default_value = "60")]
    pub run_seconds: u64,

    /// Trading symbols (comma-separated, e.g., "USDC/USDT,BTC/USDT")
    #[arg(long)]
    pub symbols: Option<String>,

    /// Generate a report after completion
    #[arg(long, default_value = "false")]
    pub report: bool,

    /// Cancel all open orders on shutdown
    #[arg(long, default_value = "true")]
    pub cancel_on_shutdown: bool,

    /// Replay data file path (for replay mode)
    #[arg(long)]
    pub replay_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // Bybit API credentials
    pub bybit_api_key: String,
    pub bybit_api_secret: String,
    pub bybit_testnet: bool,

    // Base URLs
    pub bybit_rest_base_url: String,
    pub bybit_ws_public_url: String,
    pub bybit_ws_private_url: String,

    // Trading configuration
    pub symbols: Vec<String>,
    pub max_order_size_usdc: f64,
    pub max_order_size_usdt: f64,
    pub max_position_size: f64,

    // Logging
    pub log_level: String,
    pub log_to_file: bool,
    pub log_file_path: String,

    // Runtime configuration (from CLI)
    pub mode: Mode,
    pub instrument: InstrumentType,
    pub run_seconds: u64,
    pub report: bool,
    pub cancel_on_shutdown: bool,
    pub replay_file: Option<String>,
}

impl Config {
    pub fn load(cli: Cli) -> Result<Self> {
        // Load .env file if it exists
        dotenv::dotenv().ok();

        // Parse symbols (CLI overrides env)
        let symbols = if let Some(symbols_str) = &cli.symbols {
            symbols_str
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        } else {
            env::var("SYMBOLS")
                .unwrap_or_else(|_| "USDC/USDT".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        };

        let config = Config {
            bybit_api_key: env::var("BYBIT_API_KEY")
                .context("BYBIT_API_KEY not set in environment")?,
            bybit_api_secret: env::var("BYBIT_API_SECRET")
                .context("BYBIT_API_SECRET not set in environment")?,
            bybit_testnet: env::var("BYBIT_TESTNET")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),

            bybit_rest_base_url: env::var("BYBIT_REST_BASE_URL")
                .unwrap_or_else(|_| "https://api-testnet.bybit.com".to_string()),
            bybit_ws_public_url: Self::get_ws_url(&cli.instrument)?,
            bybit_ws_private_url: env::var("BYBIT_WS_PRIVATE_URL")
                .unwrap_or_else(|_| "wss://stream-testnet.bybit.com/v5/private".to_string()),

            symbols,
            max_order_size_usdc: env::var("MAX_ORDER_SIZE_USDC")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10.0),
            max_order_size_usdt: env::var("MAX_ORDER_SIZE_USDT")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10.0),
            max_position_size: env::var("MAX_POSITION_SIZE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .unwrap_or(100.0),

            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),
            log_to_file: env::var("LOG_TO_FILE")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            log_file_path: env::var("LOG_FILE_PATH")
                .unwrap_or_else(|_| "./logs/trading.jsonl".to_string()),

            mode: cli.mode,
            instrument: cli.instrument,
            run_seconds: cli.run_seconds,
            report: cli.report,
            cancel_on_shutdown: cli.cancel_on_shutdown,
            replay_file: cli.replay_file,
        };

        Ok(config)
    }

    fn get_ws_url(instrument: &InstrumentType) -> Result<String> {
        let base = "wss://stream-testnet.bybit.com/v5/public";
        let url = match instrument {
            InstrumentType::Spot => format!("{}/spot", base),
            InstrumentType::Perp => format!("{}/linear", base),
            InstrumentType::Option => format!("{}/option", base),
        };
        Ok(url)
    }

    /// Get the Bybit category string for API calls
    pub fn get_category(&self) -> &'static str {
        match self.instrument {
            InstrumentType::Spot => "spot",
            InstrumentType::Perp => "linear",
            InstrumentType::Option => "option",
        }
    }

    /// Convert internal symbol format to Bybit format
    /// E.g., "USDC/USDT" -> "USDCUSDT"
    pub fn to_bybit_symbol(&self, symbol: &str) -> String {
        symbol.replace('/', "")
    }
}
