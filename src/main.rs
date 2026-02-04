mod config;
mod logging;

use anyhow::Result;
use clap::Parser;
use config::{Cli, Config};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let cli = Cli::parse();

    // Load configuration
    let config = Config::load(cli)?;

    // Initialize logging
    logging::init_logging(&config.log_level)?;

    // Log system start
    logging::log_system_start(&config);

    // Display configuration summary
    logging::log_info(
        "config",
        &format!(
            "Running in {:?} mode with {} instrument for {} seconds",
            config.mode, config.instrument, config.run_seconds
        ),
    );

    for symbol in &config.symbols {
        logging::log_info("config", &format!("Trading symbol: {}", symbol));
    }

    // Run the trading system
    match run_trading_system(config).await {
        Ok(_) => {
            logging::log_system_shutdown("Normal completion", true);
            Ok(())
        }
        Err(e) => {
            logging::log_error("main", &format!("Trading system error: {}", e));
            logging::log_system_shutdown(&format!("Error: {}", e), false);
            Err(e)
        }
    }
}

async fn run_trading_system(config: Config) -> Result<()> {
    logging::log_info(
        "system",
        "Trading system initialized, waiting for market data...",
    );

    // For now, just run for the configured duration
    // We'll add the actual trading components in subsequent milestones
    let run_duration = Duration::from_secs(config.run_seconds);
    
    logging::log_info(
        "system",
        &format!("System will run for {} seconds", config.run_seconds),
    );

    // Simulate running (placeholder for actual trading logic)
    let start = std::time::Instant::now();
    while start.elapsed() < run_duration {
        // Placeholder: Log a heartbeat every 5 seconds
        sleep(Duration::from_secs(5)).await;
        logging::log_info(
            "heartbeat",
            &format!(
                "System running... {} seconds elapsed",
                start.elapsed().as_secs()
            ),
        );
    }

    logging::log_info(
        "system",
        &format!("Run duration of {} seconds completed", config.run_seconds),
    );

    Ok(())
}
