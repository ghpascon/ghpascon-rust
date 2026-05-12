// Run with: cargo run --example example_logger

use ghpascon_rust::utils::logger_manager::{LogLevel, LoggerManager};

#[tokio::main]
async fn main() {
    let logger = LoggerManager::builder("example_app")
        .level(LogLevel::Debug)
        .retention_days(3)
        .build()
        .await;

    logger.debug("application starting up");
    logger.info("server listening on port 8080");
    logger.warn("memory usage above 80%");
    logger.error("failed to connect to database");
}
