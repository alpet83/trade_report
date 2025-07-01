use chrono::{DateTime, Utc, Duration, Datelike, Timelike, Weekday};
use serde_json::{Value, to_string_pretty};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration as TokioDuration};
use tracing::{info, debug};
use tracing_subscriber::EnvFilter;
use std::sync::Arc;
use std::fs;

use crate::{
    entities::task::{TaskStatus},
    entities::cache::TradesCache,
    entities::account::TradingAccount,
    entities::exchange::Exchange,
    services::task_processor::TaskProcessor,
    entities::trades_aggregator::{TradesAggregator, CalcMethod},
    common::consts::BTC_PAIR_ID,
};

static TEST_LOCK: Mutex<()> = Mutex::const_new(());

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .try_init();
}

async fn aggregate_coarse(
    interval_str: &str,
    interval: Duration,
    week_align: bool,
    processor: &TaskProcessor,
    trades_cache: Arc<TradesCache>,
    start_ts: DateTime<Utc>,
    end_ts: DateTime<Utc>,
    expected_results: &Value,
) -> Vec<Value> {
    debug!("#DBG: Testing coarse interval={} week_align={}", interval_str, week_align);
    let aggregator = TradesAggregator::new(
        trades_cache.clone(),
        start_ts,
        end_ts,
        interval,
        CalcMethod::Coarse,
        week_align,
        false,
    ).await;

    let task_id = processor.add(Box::new(aggregator.clone())).await.expect("Failed to add task");
    
    processor.wait_completed().await;
    

    processor.print_status().await;
    let completed_tasks = processor.get_completed_tasks().await;
    debug!("#DBG: Completed tasks: {} for {}", completed_tasks.len(), interval_str);
    assert!(!completed_tasks.is_empty(), "No completed tasks for {}", interval_str);

    let result = processor.get_results(task_id).await.expect("Failed to get task result");
    assert!(result.is_array(), "Result not an array for {}", interval_str);
    let trades = result.as_array().expect("Result should be array");

    let expected_trades = expected_results
        .as_array()
        .expect("Expected array")
        .iter()
        .find(|v| v["interval"].as_str().unwrap() == interval_str)
        .expect("Expected interval in results")
        ["trades"]
        .as_array()
        .expect("Expected trades array");
    assert_eq!(trades.len(), expected_trades.len(), "Unexpected number of trades for {}", interval_str);

    let trades_vec: Vec<Value> = trades.clone();
    let total_buy_amount: f32 = trades_vec.iter().filter(|t| t["buy"].as_bool().unwrap()).map(|t| t["amount"].as_f64().unwrap() as f32).sum();
    let total_sell_amount: f32 = trades_vec.iter().filter(|t| !t["buy"].as_bool().unwrap()).map(|t| t["amount"].as_f64().unwrap() as f32).sum();
    debug!("#DBG: {}: buy_amount={}, sell_amount={}", interval_str, total_buy_amount, total_sell_amount);

    if interval_str == "30d" {
        debug!("#DBG: {} trades for 30d: {:?}", trades.len(), trades.iter().map(|t| t["ts"].as_str().unwrap()).collect::<Vec<_>>());
    }

    for (i, trade) in trades.iter().enumerate() {
        let expected = &expected_trades[i];
        assert_eq!(
            trade["ts"].as_str().unwrap(),
            expected["timestamp"].as_str().unwrap(),
            "Timestamp mismatch for trade {} in {}",
            trade["trade_no"].as_str().unwrap(),
            interval_str
        );
        assert_eq!(
            trade["buy"].as_bool().unwrap(),
            expected["buy"].as_bool().unwrap(),
            "Buy mismatch for trade {} in {}",
            trade["trade_no"].as_str().unwrap(),
            interval_str
        );
        assert!(
            (trade["price"].as_f64().unwrap() - expected["price"].as_f64().unwrap()).abs() < 0.1,
            "Price mismatch for trade {} in {}: expected {}, got {}",
            trade["trade_no"].as_str().unwrap(),
            interval_str,
            expected["price"].as_f64().unwrap(),
            trade["price"].as_f64().unwrap()
        );
        assert!(
            (trade["amount"].as_f64().unwrap() - expected["amount"].as_f64().unwrap()).abs() < 0.1,
            "Amount mismatch for trade {} in {}: expected {}, got {}",
            trade["trade_no"].as_str().unwrap(),
            interval_str,
            expected["amount"].as_f64().unwrap(),
            trade["amount"].as_f64().unwrap()
        );
        assert_eq!(
            trade["trade_no"].as_str().unwrap(),
            expected["trade_no"].as_str().unwrap(),
            "Trade_no mismatch for trade {} in {}",
            trade["trade_no"].as_str().unwrap(),
            interval_str
        );
        if week_align && interval.num_seconds() >= 7 * 24 * 3600 {
            let ts = DateTime::parse_from_rfc3339(trade["ts"].as_str().unwrap())
                .unwrap()
                .with_timezone(&Utc);
            assert_eq!(ts.weekday(), Weekday::Mon, "Expected Monday alignment for trade {} in {}", trade["trade_no"].as_str().unwrap(), interval_str);
            assert_eq!(ts.hour(), 0, "Expected midnight alignment for trade {} in {}", trade["trade_no"].as_str().unwrap(), interval_str);
        } else if interval.num_seconds() >= 30 * 24 * 3600 && !week_align {
            let ts = DateTime::parse_from_rfc3339(trade["ts"].as_str().unwrap())
                .unwrap()
                .with_timezone(&Utc);
            assert_eq!(ts.day(), 1, "Expected first of month alignment for trade {} in {}", trade["trade_no"].as_str().unwrap(), interval_str);
            assert_eq!(ts.hour(), 0, "Expected midnight alignment for trade {} in {}", trade["trade_no"].as_str().unwrap(), interval_str);
        }
    }

    processor.remove(task_id).await.expect("Failed to remove task");
    processor.wait_completed().await;
    processor.reset().await;

    trades_vec
}

#[tokio::test]
async fn test_trades_aggregator_1h() {
    init_tracing();
    let _guard = TEST_LOCK.lock().await;

    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

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
    let trades_cache = Arc::new(TradesCache::new(account.clone(), BTC_PAIR_ID));

    trades_cache.import_csv("sample_trades_extended.csv".to_string())
        .expect("Failed to import sample_trades_extended.csv");

    let start_ts = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end_ts = DateTime::parse_from_rfc3339("2025-03-31T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let expected_results: Value = serde_json::from_str(
        &std::fs::read_to_string("expected_results_coarse.json").expect("Failed to read expected_results_coarse.json")
    ).expect("Failed to parse expected_results_coarse.json");

    let trades = trades_cache.get_trades(start_ts, end_ts).await.expect("Failed to get trades");
    let total_buy_amount: f32 = trades.iter().filter(|t| t.buy).map(|t| t.amount).sum();
    let total_sell_amount: f32 = trades.iter().filter(|t| !t.buy).map(|t| t.amount).sum();
    debug!("#DBG: Total buy: {}, sell: {}", total_buy_amount, total_sell_amount);

    let test_results = aggregate_coarse(
        "1h",
        Duration::hours(1),
        false,
        &processor,
        trades_cache.clone(),
        start_ts,
        end_ts,
        &expected_results,
    ).await;

    fs::write(
        "test_results_coarse_1h.json",
        to_string_pretty(&test_results).expect("Failed to serialize test results")
    ).expect("Failed to write test_results_coarse_1h.json");

    info!("#INFO: Tested coarse aggregation for 1h");
}

#[tokio::test]
async fn test_trades_aggregator_1d() {
    init_tracing();
    let _guard = TEST_LOCK.lock().await;

    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

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
    let trades_cache = Arc::new(TradesCache::new(account.clone(), BTC_PAIR_ID));

    trades_cache.import_csv("sample_trades_extended.csv".to_string())
        .expect("Failed to import sample_trades_extended.csv");

    let start_ts = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end_ts = DateTime::parse_from_rfc3339("2025-03-31T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let expected_results: Value = serde_json::from_str(
        &std::fs::read_to_string("expected_results_coarse.json").expect("Failed to read expected_results_coarse.json")
    ).expect("Failed to parse expected_results_coarse.json");

    let trades = trades_cache.get_trades(start_ts, end_ts).await.expect("Failed to get trades");
    let total_buy_amount: f32 = trades.iter().filter(|t| t.buy).map(|t| t.amount).sum();
    let total_sell_amount: f32 = trades.iter().filter(|t| !t.buy).map(|t| t.amount).sum();
    debug!("#DBG: Total buy: {}, sell: {}", total_buy_amount, total_sell_amount);

    let test_results = aggregate_coarse(
        "1d",
        Duration::days(1),
        false,
        &processor,
        trades_cache.clone(),
        start_ts,
        end_ts,
        &expected_results,
    ).await;

    fs::write(
        "test_results_coarse_1d.json",
        to_string_pretty(&test_results).expect("Failed to serialize test results")
    ).expect("Failed to write test_results_coarse_1d.json");

    info!("#INFO: Tested coarse aggregation for 1d");
}

#[tokio::test]
async fn test_trades_aggregator_7d() {
    init_tracing();
    let _guard = TEST_LOCK.lock().await;

    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

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
    let trades_cache = Arc::new(TradesCache::new(account.clone(), BTC_PAIR_ID));

    trades_cache.import_csv("sample_trades_extended.csv".to_string())
        .expect("Failed to import sample_trades_extended.csv");

    let start_ts = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end_ts = DateTime::parse_from_rfc3339("2025-03-31T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let expected_results: Value = serde_json::from_str(
        &std::fs::read_to_string("expected_results_coarse.json").expect("Failed to read expected_results_coarse.json")
    ).expect("Failed to parse expected_results_coarse.json");

    let trades = trades_cache.get_trades(start_ts, end_ts).await.expect("Failed to get trades");
    let total_buy_amount: f32 = trades.iter().filter(|t| t.buy).map(|t| t.amount).sum();
    let total_sell_amount: f32 = trades.iter().filter(|t| !t.buy).map(|t| t.amount).sum();
    debug!("#DBG: Total buy: {}, sell: {}", total_buy_amount, total_sell_amount);

    let test_results = aggregate_coarse(
        "7d",
        Duration::days(7),
        true,
        &processor,
        trades_cache.clone(),
        start_ts,
        end_ts,
        &expected_results,
    ).await;

    fs::write(
        "test_results_coarse_7d.json",
        to_string_pretty(&test_results).expect("Failed to serialize test results")
    ).expect("Failed to write test_results_coarse_7d.json");

    info!("#INFO: Tested coarse aggregation for 7d");
}

#[tokio::test]
async fn test_trades_aggregator_30d() {
    init_tracing();
    let _guard = TEST_LOCK.lock().await;

    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

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
    let trades_cache = Arc::new(TradesCache::new(account.clone(), BTC_PAIR_ID));

    trades_cache.import_csv("sample_trades_extended.csv".to_string())
        .expect("Failed to import sample_trades_extended.csv");

    let start_ts = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end_ts = DateTime::parse_from_rfc3339("2025-03-31T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let expected_results: Value = serde_json::from_str(
        &std::fs::read_to_string("expected_results_coarse.json").expect("Failed to read expected_results_coarse.json")
    ).expect("Failed to parse expected_results_coarse.json");

    let trades = trades_cache.get_trades(start_ts, end_ts).await.expect("Failed to get trades");
    let total_buy_amount: f32 = trades.iter().filter(|t| t.buy).map(|t| t.amount).sum();
    let total_sell_amount: f32 = trades.iter().filter(|t| !t.buy).map(|t| t.amount).sum();
    debug!("#DBG: Total buy: {}, sell: {}", total_buy_amount, total_sell_amount);

    let test_results = aggregate_coarse(
        "30d",
        Duration::days(30),
        false,
        &processor,
        trades_cache.clone(),
        start_ts,
        end_ts,
        &expected_results,
    ).await;

    fs::write(
        "test_results_coarse_30d.json",
        to_string_pretty(&test_results).expect("Failed to serialize test results")
    ).expect("Failed to write test_results_coarse_30d.json");

    info!("#INFO: Tested coarse aggregation for 30d");
}

#[tokio::test]
async fn test_trades_aggregator_30d_weekly() {
    init_tracing();
    let _guard = TEST_LOCK.lock().await;

    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

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
    let trades_cache = Arc::new(TradesCache::new(account.clone(), BTC_PAIR_ID));

    trades_cache.import_csv("sample_trades_extended.csv".to_string())
        .expect("Failed to import sample_trades_extended.csv");

    let start_ts = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end_ts = DateTime::parse_from_rfc3339("2025-03-31T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let expected_results: Value = serde_json::from_str(
        &std::fs::read_to_string("expected_results_coarse.json").expect("Failed to read expected_results_coarse.json")
    ).expect("Failed to parse expected_results_coarse.json");

    let trades = trades_cache.get_trades(start_ts, end_ts).await.expect("Failed to get trades");
    let total_buy_amount: f32 = trades.iter().filter(|t| t.buy).map(|t| t.amount).sum();
    let total_sell_amount: f32 = trades.iter().filter(|t| !t.buy).map(|t| t.amount).sum();
    debug!("#DBG: Total buy: {}, sell: {}", total_buy_amount, total_sell_amount);

    let test_results = aggregate_coarse(
        "30d_weekly",
        Duration::days(30),
        true,
        &processor,
        trades_cache.clone(),
        start_ts,
        end_ts,
        &expected_results,
    ).await;

    fs::write(
        "test_results_coarse_30d_weekly.json",
        to_string_pretty(&test_results).expect("Failed to serialize test results")
    ).expect("Failed to write test_results_coarse_30d_weekly.json");

    info!("#INFO: Tested coarse aggregation for 30d_weekly");
}

#[tokio::test]
async fn test_trades_aggregator_90d() {
    init_tracing();
    let _guard = TEST_LOCK.lock().await;

    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

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
    let trades_cache = Arc::new(TradesCache::new(account.clone(), BTC_PAIR_ID));

    trades_cache.import_csv("sample_trades_extended.csv".to_string())
        .expect("Failed to import sample_trades_extended.csv");

    let start_ts = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end_ts = DateTime::parse_from_rfc3339("2025-03-31T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let expected_results: Value = serde_json::from_str(
        &std::fs::read_to_string("expected_results_coarse.json").expect("Failed to read expected_results_coarse.json")
    ).expect("Failed to parse expected_results_coarse.json");

    let trades = trades_cache.get_trades(start_ts, end_ts).await.expect("Failed to get trades");
    let total_buy_amount: f32 = trades.iter().filter(|t| t.buy).map(|t| t.amount).sum();
    let total_sell_amount: f32 = trades.iter().filter(|t| !t.buy).map(|t| t.amount).sum();
    debug!("#DBG: Total buy: {}, sell: {}", total_buy_amount, total_sell_amount);

    let test_results = aggregate_coarse(
        "90d",
        Duration::days(90),
        true,
        &processor,
        trades_cache.clone(),
        start_ts,
        end_ts,
        &expected_results,
    ).await;

    fs::write(
        "test_results_coarse_90d.json",
        to_string_pretty(&test_results).expect("Failed to serialize test results")
    ).expect("Failed to write test_results_coarse_90d.json");

    info!("#INFO: Tested coarse aggregation for 90d");
}

#[tokio::test]
async fn test_trades_aggregator_365d() {
    init_tracing();
    let _guard = TEST_LOCK.lock().await;

    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

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
    let trades_cache = Arc::new(TradesCache::new(account.clone(), BTC_PAIR_ID));

    trades_cache.import_csv("sample_trades_extended.csv".to_string())
        .expect("Failed to import sample_trades_extended.csv");

    let start_ts = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end_ts = DateTime::parse_from_rfc3339("2025-03-31T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let expected_results: Value = serde_json::from_str(
        &std::fs::read_to_string("expected_results_coarse.json").expect("Failed to read expected_results_coarse.json")
    ).expect("Failed to parse expected_results_coarse.json");

    let trades = trades_cache.get_trades(start_ts, end_ts).await.expect("Failed to get trades");
    let total_buy_amount: f32 = trades.iter().filter(|t| t.buy).map(|t| t.amount).sum();
    let total_sell_amount: f32 = trades.iter().filter(|t| !t.buy).map(|t| t.amount).sum();
    debug!("#DBG: Total buy: {}, sell: {}", total_buy_amount, total_sell_amount);

    let test_results = aggregate_coarse(
        "365d",
        Duration::days(365),
        true,
        &processor,
        trades_cache.clone(),
        start_ts,
        end_ts,
        &expected_results,
    ).await;

    fs::write(
        "test_results_coarse_365d.json",
        to_string_pretty(&test_results).expect("Failed to serialize test results")
    ).expect("Failed to write test_results_coarse_365d.json");

    info!("#INFO: Tested coarse aggregation for 365d");
}

#[tokio::test]
async fn test_trades_aggregator_precise() {
    init_tracing();
    let _guard = TEST_LOCK.lock().await;

    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

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
    let trades_cache = Arc::new(TradesCache::new(account.clone(), BTC_PAIR_ID));

    trades_cache.import_csv("sample_trades_extended.csv".to_string())
        .expect("Failed to import sample_trades_extended.csv");

    let start_ts = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end_ts = DateTime::parse_from_rfc3339("2025-03-31T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let expected_results: Value = serde_json::from_str(
        &std::fs::read_to_string("expected_results_precise.json").expect("Failed to read expected_results_precise.json")
    ).expect("Failed to parse expected_results_precise.json");

    debug!("#DBG: Testing precise aggregation");
    let mut aggregator = TradesAggregator::new(
        trades_cache.clone(),
        start_ts,
        end_ts,
        Duration::days(7),
        CalcMethod::Precise,
        false,
        false,
    ).await;

    let task_id = processor.add(Box::new(aggregator)).await.expect("Failed to add task");
    debug!("#DBG: Waiting 5000ms for task (temporary)");
    processor.wait_completed().await;
    sleep(TokioDuration::from_millis(5000)).await;

    processor.print_status().await;
    let completed_tasks = processor.get_completed_tasks().await;
    debug!("#DBG: Completed tasks: {}", completed_tasks.len());
    assert!(!completed_tasks.is_empty(), "No completed tasks for precise aggregation");

    let result = processor.get_results(task_id).await.expect("Failed to get task result");
    assert!(result.is_array(), "Result not an array for precise aggregation");
    let trades = result.as_array().expect("Result should be array");

    let expected_trades = expected_results.as_array().expect("Expected precise results array");
    assert_eq!(trades.len(), expected_trades.len(), "Unexpected number of trades for precise aggregation");

    let total_buy_amount: f32 = trades.iter().filter(|t| t["buy"].as_bool().unwrap()).map(|t| t["amount"].as_f64().unwrap() as f32).sum();
    let total_sell_amount: f32 = trades.iter().filter(|t| !t["buy"].as_bool().unwrap()).map(|t| t["amount"].as_f64().unwrap() as f32).sum();
    debug!("#DBG: Precise: buy_amount={}, sell_amount={}", total_buy_amount, total_sell_amount);

    for (i, trade) in trades.iter().enumerate() {
        let expected = &expected_trades[i];
        assert_eq!(
            trade["ts"].as_str().unwrap(),
            expected["timestamp"].as_str().unwrap(),
            "Timestamp mismatch for trade {}",
            trade["trade_no"].as_str().unwrap()
        );
        assert_eq!(
            trade["buy"].as_bool().unwrap(),
            expected["buy"].as_bool().unwrap(),
            "Buy mismatch for trade {}",
            trade["trade_no"].as_str().unwrap()
        );
        assert!(
            (trade["price"].as_f64().unwrap() - expected["price"].as_f64().unwrap()).abs() < 0.1,
            "Price mismatch for trade {}: expected {}, got {}",
            trade["trade_no"].as_str().unwrap(),
            expected["price"].as_f64().unwrap(),
            trade["price"].as_f64().unwrap()
        );
        assert!(
            (trade["amount"].as_f64().unwrap() - expected["amount"].as_f64().unwrap()).abs() < 0.1,
            "Amount mismatch for trade {}: expected {}, got {}",
            trade["trade_no"].as_str().unwrap(),
            expected["amount"].as_f64().unwrap(),
            trade["amount"].as_f64().unwrap()
        );
        assert_eq!(
            trade["trade_no"].as_str().unwrap(),
            expected["trade_no"].as_str().unwrap(),
            "Trade_no mismatch for trade {}",
            trade["trade_no"].as_str().unwrap()
        );
    }

    let found_task = processor.find_task(task_id).await;
    assert!(found_task.is_some(), "Expected to find task with id={}", task_id);
    if let Some(t) = found_task {
        let t_read = t.read().await;
        assert_eq!(t_read.id(), task_id, "Expected task ID to match");
        assert_eq!(t_read.result(), result, "Expected task result to match");
        assert_eq!(t_read.status(), TaskStatus::Completed, "Expected task status to be Completed");
    }

    processor.remove(task_id).await.expect("Failed to remove task");
    processor.wait_completed().await;
    processor.reset().await;

    info!("#INFO: Tested precise aggregation");
}

#[tokio::test]
async fn test_aggregate_coarse_correctness() {
    init_tracing();
    let _guard = TEST_LOCK.lock().await;

    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

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
    let cache = Arc::new(TradesCache::new(account.clone(), BTC_PAIR_ID));
    cache.import_csv("sample_trades_extended.csv".to_string())
        .expect("Failed to import sample_trades_extended.csv");

    let start_ts = DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end_ts = DateTime::parse_from_rfc3339("2025-03-31T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);

    let expected_results: Value = serde_json::from_str(
        &std::fs::read_to_string("expected_results_coarse.json").expect("Failed to read expected_results_coarse.json")
    ).expect("Failed to parse expected_results_coarse.json");

    let trades = cache.get_trades(start_ts, end_ts).await.expect("Failed to get trades");
    let total_buy_amount: f32 = trades.iter().filter(|t| t.buy).map(|t| t.amount).sum();
    let total_sell_amount: f32 = trades.iter().filter(|t| !t.buy).map(|t| t.amount).sum();
    debug!("#DBG: Total buy: {}, sell: {}", total_buy_amount, total_sell_amount);

    let test_results = aggregate_coarse(
        "7d",
        Duration::days(7),
        true,
        &processor,
        cache.clone(),
        start_ts,
        end_ts,
        &expected_results,
    ).await;

    let trades = test_results.clone();
    let expected_trades = expected_results
        .as_array()
        .expect("Expected array")
        .iter()
        .find(|v| v["interval"].as_str().unwrap() == "7d")
        .expect("Expected 7d interval in results")
        ["trades"]
        .as_array()
        .expect("Expected trades array");

    let buy_year_trade = trades.iter().find(|t| t["trade_no"].as_str().unwrap() == "buy_year_1:buy_year_4").unwrap();
    let expected_buy_year = expected_trades.iter().find(|t| t["trade_no"].as_str().unwrap() == "buy_year_1:buy_year_4").unwrap();
    assert!(
        (buy_year_trade["amount"].as_f64().unwrap() - expected_buy_year["amount"].as_f64().unwrap()).abs() < 0.1,
        "Expected amount={} for buy_year_1:buy_year_4",
        expected_buy_year["amount"].as_f64().unwrap()
    );
    assert!(
        (buy_year_trade["price"].as_f64().unwrap() - expected_buy_year["price"].as_f64().unwrap()).abs() < 0.1,
        "Expected price={} for buy_year_1:buy_year_4",
        expected_buy_year["price"].as_f64().unwrap()
    );

    fs::write(
        "test_results_coarse_correctness.json",
        to_string_pretty(&test_results).expect("Failed to serialize test results")
    ).expect("Failed to write test_results_coarse_correctness.json");

    info!("#INFO: Tested coarse aggregation correctness for 7d");
}