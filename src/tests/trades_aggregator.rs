// /src/tests/trades_aggregator.rs
// Modified: 2025-06-24 14:15:00 EEST

use chrono::{DateTime, Utc, Duration};
use std::sync::Arc;
use tracing::{debug, info};
use tracing_subscriber::EnvFilter;

use crate::{
    entities::trade::Trade,
    entities::cache::TradesCache,
    entities::account::TradingAccount,
    entities::exchange::Exchange,
    entities::trades_aggregator::{TradesAggregator, CalcMethod},
    common::consts::BTC_PAIR_ID,
};

// Initializes tracing for test output
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("debug".parse().unwrap()))
        .try_init();
}

// Tests TradesAggregator with Coarse mode using sample_trades.csv
#[tokio::test]
async fn test_trades_aggregator_coarse() {
    init_tracing();

    // Create test account
    let account = Arc::new(TradingAccount::new(
        11223344,
        "test".to_string(),
        Arc::new(Exchange {
            name: "bitmex".to_string(),
            candles_cache: std::collections::HashMap::new(),
            price_caches: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }),
        true,
    ));

    // Create TradesCache
    let cache = TradesCache::new(account, BTC_PAIR_ID);

    // Import trades from sample_trades.csv
    cache.import_csv("sample_trades.csv".to_string()).expect("Failed to import CSV");

    // Create TradesAggregator
    let start_ts = DateTime::parse_from_rfc3339("2025-06-24T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end_ts = DateTime::parse_from_rfc3339("2025-06-26T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let mut aggregator = TradesAggregator::new(
        Arc::new(cache),
        start_ts,
        end_ts,
        Duration::days(1),
        CalcMethod::Coarse,
        false,
    )
    .await;

    // Run aggregation
    aggregator.aggregate_coarse().await.expect("Aggregation failed");

    // Debug output of results
    for trade in &aggregator.results {
        debug!("#DBG: Virtual trade: date={}, buy={}, price={}, amount={}, flags={}, trade_no={}", 
            trade.ts.date_naive(), trade.buy, trade.price, trade.amount, trade.flags, trade.trade_no);
    }

    // Verify results
    let results = &aggregator.results;
    assert_eq!(results.len(), 4, "Expected 4 virtual trades (2 buys, 2 sells)");

    // Expected results for 2025-06-24
    let buy_24 = results.iter().find(|t| t.ts.date_naive() == chrono::NaiveDate::from_ymd_opt(2025, 6, 24).unwrap() && t.buy);
    let sell_24 = results.iter().find(|t| t.ts.date_naive() == chrono::NaiveDate::from_ymd_opt(2025, 6, 24).unwrap() && !t.buy);
    assert!(buy_24.is_some(), "Expected buy trade for 2025-06-24");
    assert!(sell_24.is_some(), "Expected sell trade for 2025-06-24");
    let buy_24 = buy_24.unwrap();
    let sell_24 = sell_24.unwrap();
    assert_eq!(buy_24.flags, 4, "Expected 4 source trades for buy on 2025-06-24");
    assert_eq!(sell_24.flags, 3, "Expected 3 source trades for sell on 2025-06-24");
    assert_eq!(buy_24.trade_no, "buy_1:buy_4", "Incorrect trade_no for buy on 2025-06-24");
    assert_eq!(sell_24.trade_no, "sell_1:sell_3", "Incorrect trade_no for sell on 2025-06-24");
    assert!((buy_24.amount - 24111.2).abs() < 0.001, "Incorrect buy volume for 2025-06-24: {}", buy_24.amount);
    assert!((buy_24.price - 86293.9).abs() < 0.001, "Incorrect buy price for 2025-06-24: {}", buy_24.price);
    assert!((sell_24.amount - 20038.4).abs() < 0.001, "Incorrect sell volume for 2025-06-24: {}", sell_24.amount);
    assert!((sell_24.price - 10382.2).abs() < 0.001, "Incorrect sell price for 2025-06-24: {}", sell_24.price);

    // Expected results for 2025-06-25
    let buy_25 = results.iter().find(|t| t.ts.date_naive() == chrono::NaiveDate::from_ymd_opt(2025, 6, 25).unwrap() && t.buy);
    let sell_25 = results.iter().find(|t| t.ts.date_naive() == chrono::NaiveDate::from_ymd_opt(2025, 6, 25).unwrap() && !t.buy);
    assert!(buy_25.is_some(), "Expected buy trade for 2025-06-25");
    assert!(sell_25.is_some(), "Expected sell trade for 2025-06-25");
    let buy_25 = buy_25.unwrap();
    let sell_25 = sell_25.unwrap();
    assert_eq!(buy_25.flags, 1, "Expected 1 source trade for buy on 2025-06-25");
    assert_eq!(sell_25.flags, 2, "Expected 2 source trades for sell on 2025-06-25");
    assert_eq!(buy_25.trade_no, "buy_5", "Incorrect trade_no for buy on 2025-06-25");
    assert_eq!(sell_25.trade_no, "sell_4:sell_5", "Incorrect trade_no for sell on 2025-06-25");
    assert!((buy_25.amount - 1268.9).abs() < 0.001, "Incorrect buy volume for 2025-06-25: {}", buy_25.amount);
    assert!((buy_25.price - 38664.2).abs() < 0.001, "Incorrect buy price for 2025-06-25: {}", buy_25.price);
    assert!((sell_25.amount - 8492.6).abs() < 0.001, "Incorrect sell volume for 2025-06-25: {}", sell_25.amount);
    assert!((sell_25.price - 67466.7).abs() < 0.001, "Incorrect sell price for 2025-06-25: {}", sell_25.price);

    info!("#INFO: Successfully tested TradesAggregator in Coarse mode");
}