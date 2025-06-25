// /src/main.rs
// Modified: 2025-06-25 16:26 EEST

use axum::{Router, routing::get};
use tokio::net::TcpListener;
use tracing::{info, error, debug};
use tracing_subscriber::EnvFilter;
use trade_report::{
    api::{report, rtm, task}, // Added task
    config::Config,
    entities::account,
    db::mysql::MySqlDataSource,
    services::task_processor::TaskProcessor,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    info!("Starting Trade Report v0.3.0");

    // Load configuration
    debug!("Loading configuration");
    let config = Config::load().map_err(|e| format!("Failed to load config: {}", e))?;

    // Initialize MySQL pool
    debug!("Initializing MySQL pool");
    let pool = MySqlDataSource::init_db_conn(&config.mysql.url)
        .await
        .map_err(|e| format!("Failed to connect to MySQL: {}", e))?;

    // Initialize TaskProcessor
    debug!("Initializing TaskProcessor");
    TaskProcessor::init();
    info!("TaskProcessor initialized");
    
    // Initialize TradingAccountManager
    debug!("Creating TradingAccountManager");
    account::create_account_manager();
    let manager = account::get_account_manager();
    debug!("Acquiring write lock for initialization");
    let mut guard = manager.write().await;
    guard
        .initialize()
        .await
        .map_err(|e| format!("Failed to initialize TradingAccountManager: {}", e))?;
    drop(guard);
    info!("TradingAccountManager initialized");

    // Set up router with API routes
    debug!("Setting up Axum router");
    let app = Router::new()
        .route("/", get(|| async { "Trade Report v0.3.0" }))
        .nest("/api", report::routes())
        .nest("/rtm", rtm::routes())
        .nest("/task", task::routes()); // Added task routes

    // Start server
    debug!("Starting server on port {}", config.server.api_port);
    let listener = match TcpListener::bind(format!("0.0.0.0:{}", config.server.api_port)).await {
        Ok(listener) => listener,
        Err(e) => {
            error!("Failed to bind to port {}: {}", config.server.api_port, e);
            return Err(e.into());
        }
    };
    info!("Server running on http://0.0.0.0:{}", config.server.api_port);
    axum::serve(listener, app).await?;

    Ok(())
}