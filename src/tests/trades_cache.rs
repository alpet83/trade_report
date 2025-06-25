// /src/tests/trades_cache.rs
// Modified: 2025-06-24 10:41:00 EEST

use chrono::{DateTime, Utc, Duration};
use rand::{rng, Rng};
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use tracing::{debug};
use tracing_subscriber::EnvFilter;

use crate::{
    entities::trade::Trade,
    entities::cache::TradesCache,
    entities::account::TradingAccount,
    entities::exchange::Exchange,
};

// Initializes tracing for test output
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .try_init();
}

// Tests TradesCache by importing trades from a CSV file and retrieving them
#[test]
fn test_trades_cache_import_and_get() {
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

    // Create TradesCache
    let cache = TradesCache::new(account, 1);

    // Generate test_trades.csv with 10 random trades
    let mut rng = rng();
    let base_ts = Utc::now();
    let mut trades = Vec::new();
    let mut csv_content = String::new();

    for i in 1..=10 {
        let ts = base_ts + Duration::minutes(rng.random_range(0..1440));
        let buy = rng.random_bool(0.5);
        let price = rng.random_range(0.01..100000.0);
        let amount = rng.random_range(0.1..10000.0);
        let trade_no = format!("trade_{}", i);

        trades.push(Trade {
            ts,
            pair_id: 1,
            buy,
            price,
            amount,
            trade_no: trade_no.clone(),
            order_id: 0,
            position: 0.0,
            rpnl: 0.0,
            flags: 0,
            comission: 0.0,
        });

        csv_content.push_str(&format!(
            "{},{},{},{},{}\n",
            ts.to_rfc3339(),
            buy,
            price,
            amount,
            trade_no
        ));
    }

    // Write CSV file
    let file_name = "test_trades.csv";
    File::create(file_name)
        .and_then(|mut file| file.write_all(csv_content.as_bytes()))
        .expect("Failed to create test_trades.csv");

    // Import CSV
    cache.import_csv(file_name.to_string()).expect("Failed to import CSV");

    // Verify imported trades
    let imported_trades = cache.data.get(&1).expect("No trades found in cache");
    assert_eq!(imported_trades.len(), 10, "Expected 10 trades in cache");

    // Verify get_trade for each trade
    for trade in trades.iter() {
        let retrieved = cache.get_trade(&trade.trade_no);
        assert!(retrieved.is_some(), "Trade {} not found", trade.trade_no);
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.ts, trade.ts, "Timestamp mismatch for {}", trade.trade_no);
        assert_eq!(retrieved.buy, trade.buy, "Buy mismatch for {}", trade.trade_no);
        assert_eq!(retrieved.price, trade.price, "Price mismatch for {}", trade.trade_no);
        assert_eq!(retrieved.amount, trade.amount, "Amount mismatch for {}", trade.trade_no);
        assert_eq!(retrieved.trade_no, trade.trade_no, "Trade_no mismatch for {}", trade.trade_no);
    }

    debug!("Successfully tested TradesCache import and get_trade");
}