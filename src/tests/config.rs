use sqlx::{MySqlPool, Row};
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::{config::{Config, BotConfigMap}, entities::account::{get_account_manager, create_account_manager, TradingAccountManager}};

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .try_init();
}

#[sqlx::test]
async fn test_bot_config_map_load(pool: MySqlPool) {
    init_tracing();

    // Mock config__table_map
    sqlx::query("CREATE TABLE config__table_map (table_key VARCHAR(16), table_name VARCHAR(16), applicant VARCHAR(16))")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO config__table_map (table_key, table_name, applicant) VALUES ('config', 'config__binance', 'binance_bot')")
        .execute(&pool)
        .await
        .unwrap();

    // Mock config__binance
    sqlx::query("CREATE TABLE config__binance (account_id INT, param VARCHAR(32), value VARCHAR(64), PRIMARY KEY (account_id, param))")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO config__binance (account_id, param, value) VALUES (35822956, 'exchange', 'Binance'), (35822956, 'monitor_enabled', '1')")
        .execute(&pool)
        .await
        .unwrap();

    // Create singleton and initialize manager
    create_account_manager();
    let manager = get_account_manager();
    let mut guard = manager.lock().await;
    guard.initialize(&pool).await.expect("Failed to initialize manager");

    // Assert results
    let guard = manager.lock().await;
    assert_eq!(guard.list_accounts().len(), 1, "Expected 1 account");

    let account = guard.get_account("35822956").expect("Account not found");
    assert_eq!(account.account_id, "35822956");
    assert_eq!(account.applicant, "binance_bot");
    assert_eq!(account.exchange.name, "Binance");
    assert_eq!(account.monitor_enabled, true);

    let account = guard.find_account("binance_bot").expect("Account not found by applicant");
    assert_eq!(account.account_id, "35822956");

    info!("Successfully tested bot config map loading");
}

#[test]
fn test_config_load() {
    init_tracing();

    // Create temporary config.toml
    let config_content = r#"
        [mysql]
        url = "mysql://test_user:test_pass@localhost/test_trading"

        [server]
        api_port = 8080

        [logging]
        log_dir = "test_logs"
    "#;
    std::fs::write("test_config.toml", config_content).expect("Failed to write test_config.toml");

    // Mock environment to use test_config.toml
    std::env::set_var("CONFIG_PATH", "test_config.toml");

    // Load config
    let config = Config::load().expect("Failed to load config");

    // Assert results
    assert_eq!(config.mysql.url, "mysql://test_user:test_pass@localhost/test_trading");
    assert_eq!(config.server.api_port, 8080);
    assert_eq!(config.log_dir(), std::path::PathBuf::from("test_logs"));

    // Clean up
    std::fs::remove_file("test_config.toml").expect("Failed to remove test_config.toml");
    std::env::remove_var("CONFIG_PATH");

    info!("Successfully tested config loading");
}