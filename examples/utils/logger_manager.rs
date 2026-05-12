// Run with: cargo run --example example_logger

// Importa os macros sobrescritos — println!/eprintln! viram DEBUG no logger global
use ghpascon_rust::utils::logger_manager::{LogLevel, LoggerManager};
use ghpascon_rust::{eprintln, println};

#[tokio::main]
async fn main() {
    let logger = LoggerManager::builder("example_app")
        .level(LogLevel::Debug)
        .retention_days(3)
        .build()
        .await;
    logger.set_as_global();

    logger.debug("application starting up");
    logger.info("server listening on port 8080");
    logger.warn("memory usage above 80%");
    logger.error("failed to connect to database");

    // A partir daqui, println!/eprintln! vão para o logger como DEBUG
    println!("este println vai para o log como DEBUG");
    eprintln!("este eprintln também vai para o log como DEBUG");
    println!("valor formatado: {} + {} = {}", 1, 2, 3);
    println!("multilinha:\nlinha 1\nlinha 2\nlinha 3");
}
