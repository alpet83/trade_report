use sqlx::{MySqlPool, Row};
use tracing::{info, error};
use tracing_subscriber;

use crate::{config::Config, db::mysql::TradeDataSource};

// Initialize tracing for test output
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .try_init();
}

#[tokio::test]
async fn test_mysql_connection() {
    init_tracing();

    // Load configuration
    let config = match Config::load() {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load config: {}", e);
            panic!("Config loading failed");
        }
    };
    info!("Using MySQL URL: {}", config.mysql_url);

    // Create MySQL pool
    let pool = match MySqlPool::connect(&config.mysql_url).await {
        Ok(pool) => pool,
        Err(e) => {
            error!("Failed to connect to MySQL: {}", e);
            panic!("MySQL connection failed");
        }
    };

    // Query list of tables
    let tables: Vec<String> = match sqlx::query("SHOW TABLES")
        .fetch_all(&pool)
        .await
    {
        Ok(rows) => rows.into_iter().map(|row| row.get(0)).collect(),
        Err(e) => {
            error!("Failed to query tables: {}", e);
            panic!("Table query failed");
        }
    };

    // Log tables
    info!("Found tables: {:?}", tables);

    // Assert that tables are not empty
    assert!(!tables.is_empty(), "No tables found in database");

    // Close pool
    pool.close().await;
}

#[sqlx::test]
async fn test_mysql_connection_mock(pool: MySqlPool) {
    init_tracing();

    // Mock SHOW TABLES result
    let mock_tables = vec!["bitmex__trades".to_string(), "bitmex__config__reports".to_string()];
    sqlx::query("CREATE TABLE mock_tables (name VARCHAR(255))")
        .execute(&pool)
        .await
        .expect("Failed to create mock table");
    for table in &mock_tables {
        sqlx::query("INSERT INTO mock_tables (name) VALUES (?)")
            .bind(table)
            .execute(&pool)
            .await
            .expect("Failed to insert mock table");
    }

    // Query list of tables (mock)
    let tables: Vec<String> = sqlx::query("SELECT name FROM mock_tables")
        .fetch_all(&pool)
        .await
        .expect("Failed to query mock tables")
        .into_iter()
        .map(|row| row.get(0))
        .collect();

    // Log tables
    info!("Mock tables: {:?}", tables);

    // Assert mock tables
    assert_eq!(tables, mock_tables, "Mock tables do not match expected");
}