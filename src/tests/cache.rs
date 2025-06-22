// /src/tests/cache.rs
// Modified: 2025-06-22 11:45:00 EEST

use chrono::{DateTime, Utc, Duration};
use std::sync::Arc;
use tracing::{info};
use tracing_subscriber::EnvFilter;
use async_trait::async_trait;

use crate::{
    entities::public_data::{Candle, PublicDataSource},
    entities::exchange::Exchange,
    entities::cache::PriceCache,
    db::mysql::MySqlDataSource,
    logs::app_error::AppError,
};

// Initializes tracing for test output
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .try_init();
}

// Mock implementation of PublicDataSource for testing
struct MockPublicDataSource {
    candles: Vec<Candle>,
}

#[async_trait]
impl PublicDataSource for MockPublicDataSource {
    // Mocks candle loading for a given time range and exchange
    async fn load_candles(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        _exchange: &str,
        _pair_id: Option<i32>,
    ) -> Result<Vec<Candle>, AppError> {
        Ok(self.candles.iter()
            .filter(|c| c.ts >= start && c.ts <= end)
            .cloned()
            .collect())
    }

    // Mocks ticker retrieval (not used in tests)
    async fn get_ticker(
        &self,
        _exchange: &str,
        _pair_id: i32,
    ) -> Result<String, AppError> {
        Ok("BTCUSD".to_string())
    }
}

// Tests PriceCache functionality
#[tokio::test]
async fn test_price_cache_load_prefetch_and_get_vwap() {
    init_tracing();

    // Initialize mock database
    let start_ts = Utc::now();
    let end_ts = start_ts + Duration::hours(4);
    let candles = vec![
        Candle {
            ts: start_ts,
            open: 80000.0,
            high: 81000.0,
            low: 79000.0,
            close: 80500.0,
            volume: 100.0,
        },
        Candle {
            ts: start_ts + Duration::hours(1),
            open: 80500.0,
            high: 81500.0,
            low: 79500.0,
            close: 81000.0,
            volume: 150.0,
        },
    ];
    let mock_db = Arc::new(MySqlDataSource {
        pool: sqlx::Pool::connect("mysql://test").await.unwrap(), // Mock pool
    });
    MySqlDataSource::init_db_conn_with_mock(mock_db.clone()).await;

    let exchange = Arc::new(Exchange::new("bitmex".to_string()).await);
    let cache = exchange.get_price_cache(Some(1)).await;

    // Test that cache is prefetched
    let hour_timestamp = (start_ts.timestamp() / 3600) as i32;
    let vwap = cache.data.get(&hour_timestamp)
        .expect("VWAP not cached");
    let expected_vwap = (80000.0 + 81000.0 + 79000.0 + 80500.0) / 4.0; // Average price
    assert!((vwap - expected_vwap).abs() < 0.001, "Expected VWAP ≈ {}, got {}", expected_vwap, vwap);

    // Test get_vwap
    let vwap = cache.get_vwap(&MySqlDataSource::db_conn(), start_ts)
        .await
        .expect("Failed to get VWAP");
    assert!((vwap - expected_vwap).abs() < 0.001, "Expected VWAP ≈ {}, got {}", expected_vwap, vwap);

    // Test get_vwap with missing timestamp (fallback to last available)
    let missing_ts = end_ts + Duration::hours(2);
    let vwap = cache.get_vwap(&MySqlDataSource::db_conn(), missing_ts)
        .await
        .expect("Failed to get VWAP for missing timestamp");
    assert!((vwap - expected_vwap).abs() < 0.001, "Expected VWAP ≈ {}, got {}", expected_vwap, vwap);

    // Test error case (no candles)
    let empty_db = Arc::new(MySqlDataSource {
        pool: sqlx::Pool::connect("mysql://test").await.unwrap(), // Mock pool
    });
    MySqlDataSource::init_db_conn_with_mock(empty_db.clone()).await;
    let exchange = Arc::new(Exchange::new("bitmex".to_string()).await);
    let cache = exchange.get_price_cache(Some(1)).await;
    let result = cache.get_vwap(&MySqlDataSource::db_conn(), start_ts).await;
    assert!(result.is_err(), "Expected error for empty candles");
    assert_eq!(result.unwrap_err().to_string(), "Internal error: No VWAP data available for timestamp ".to_string() + &start_ts.to_string());

    info!("Successfully tested PriceCache load_prefetch and get_vwap");
}

// Mock initialization for tests
impl MySqlDataSource {
    // Initializes the global MySqlDataSource singleton with a mock for testing
    async fn init_db_conn_with_mock(db: Arc<MySqlDataSource>) {
        DB_CONN.set(db).expect("Database connection already initialized");
    }
}