// /src/tests/trades_aggregator.rs
// Modified: 2025-06-25 09:58 EEST

use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration, Datelike, Timelike, Weekday, Months};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, error, info};
use tracing_subscriber::EnvFilter;
use axum::{http::{Request, StatusCode}, body::Body};
use tower::ServiceExt;
use axum::body::to_bytes; // Added
use serde_json::json;

use crate::{
    entities::trade::Trade,
    entities::cache::TradesCache,
    entities::task::{Task, TaskStatus, TaskBase},
    entities::account::TradingAccount,
    entities::exchange::Exchange,
    api::rtm,
    common::math::auto_round,
    common::consts::BTC_PAIR_ID,
};

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .try_init();
}

// Тест для проверки эндпоинта /trades_aggregated
#[tokio::test]
async fn test_trades_aggregated_endpoint() {
    init_tracing();

    // Create test account
    let account = Arc::new(TradingAccount::new(
        1,
        "test".to_string(),
        Arc::new(Exchange {
            name: "bitmex".to_string(),
            candles_cache: std::collections::HashMap::new(),
            price_caches: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }),
        true,
    ));

    // Create TradesCache and import sample_trades.csv
    let cache = TradesCache::new(account.clone(), BTC_PAIR_ID);
    cache.import_csv("sample_trades.csv".to_string())
        .expect("Failed to import sample_trades.csv");

    // Load expected results
    let expected_results: Value = serde_json::from_str(
        &std::fs::read_to_string("expected_results.json").expect("Failed to read expected_results.json")
    ).expect("Failed to parse expected_results.json");

    // Create test router
    let app = rtm::routes();

    // Test 1: Coarse aggregation with 7d interval, week_align=true
    let query = format!(
        "/trades_aggregated?account_id=1&exchange=bitmex&start_ts=2024-12-01T00:00:00Z&end_ts=2025-01-28T00:00:00Z&coarse_interval=7d&week_align=1"
    );
    let request = Request::builder()
        .method("GET")
        .uri(query)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "Expected OK status for coarse aggregation");

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let trades: Vec<Trade> = serde_json::from_slice(&body_bytes).expect("Failed to parse response");
    let expected_trades = expected_results["weekly"].as_array().expect("Expected weekly results array");
    assert_eq!(trades.len(), expected_trades.len(), "Unexpected number of trades for weekly aggregation");
    for (i, trade) in trades.iter().enumerate() {
        let expected = &expected_trades[i];
        assert_eq!(
            trade.ts.to_rfc3339(),
            expected["timestamp"].as_str().unwrap(),
            "Timestamp mismatch for trade {}",
            trade.trade_no
        );
        assert_eq!(
            trade.buy,
            expected["buy"].as_bool().unwrap(),
            "Buy mismatch for trade {}",
            trade.trade_no
        );
        assert_eq!(
            trade.price,
            expected["price"].as_f64().unwrap() as f32,
            "Price mismatch for trade {}",
            trade.trade_no
        );
        assert_eq!(
            trade.amount,
            expected["amount"].as_f64().unwrap() as f32,
            "Amount mismatch for trade {}",
            trade.trade_no
        );
        assert_eq!(
            trade.trade_no,
            expected["trade_no"].as_str().unwrap(),
            "Trade_no mismatch for trade {}",
            trade.trade_no
        );
    }
    info!("Successfully tested /trades_aggregated with coarse_interval=7d");

    // Test 2: Precise aggregation with precise_comb=1
    let query = format!(
        "/trades_aggregated?account_id=1&exchange=bitmex&start_ts=2024-12-01T00:00:00Z&end_ts=2025-01-28T00:00:00Z&precise_comb=1"
    );
    let request = Request::builder()
        .method("GET")
        .uri(query)
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK, "Expected OK status for precise aggregation");

    let body_bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let trades: Vec<Trade> = serde_json::from_slice(&body_bytes).expect("Failed to parse response");
    assert!(!trades.is_empty(), "Expected non-empty trades for precise aggregation");
    let mut last_buy = None;
    for trade in &trades {
        if let Some(buy) = last_buy {
            assert_ne!(buy, trade.buy, "Expected alternating buy/sell in precise aggregation");
        }
        last_buy = Some(trade.buy);
    }
    info!("Successfully tested /trades_aggregated with precise_comb=1");
}