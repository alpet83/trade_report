use chrono::{DateTime, Utc, Duration};
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration as TokioDuration};
use tracing::{info, debug};
use tracing_subscriber::EnvFilter;
use delegate::delegate;
use std::sync::Arc;

use crate::{
    entities::task::{TaskStatus, Task, TaskBase},
    entities::cache::TradesCache,
    entities::account::TradingAccount,
    entities::exchange::Exchange,
    services::task_processor::TaskProcessor,
    entities::trades_aggregator::{TradesAggregator, CalcMethod},
    common::consts::BTC_PAIR_ID,
};

// Global mutex for ensuring sequential test execution
static TEST_LOCK: Mutex<()> = Mutex::const_new(());

// Initializes tracing for test output
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .try_init();
}

// Mock task for testing TaskProcessor
#[derive(Debug, Clone)]
struct TestTask {
    base: TaskBase,
}

impl TestTask {
    // Creates a new TestTask instance
    pub fn new() -> Self {
        TestTask {
            base: TaskBase::new(),
        }
    }
}

#[async_trait::async_trait]
impl Task for TestTask {
    delegate! {
        to self.base {
            fn status(&self) -> TaskStatus;
            fn set_status(&mut self, status: TaskStatus);
            fn result(&self) -> serde_json::Value;
            fn set_result(&mut self, result: serde_json::Value);
            fn start_at(&self) -> DateTime<Utc>;
            fn set_start_at(&mut self, start_at: DateTime<Utc>);
            fn id(&self) -> u32;
            fn set_id(&mut self, id: u32);
        }
    }

    // Initializes the task (no-op for test)
    async fn init(&mut self) -> Result<(), String> {
        debug!("#DBG: Initializing TestTask");
        Ok(())
    }

    // Executes the task, always returning Completed
    async fn run(&mut self) -> Result<TaskStatus, String> {
        debug!("#DBG: Running TestTask");
        self.set_result(Value::String("Completed successfully".to_string()));
        self.set_status(TaskStatus::Completed);
        Ok(TaskStatus::Completed)
    }

    // Releases resources (no-op for test)
    async fn release(&mut self) -> Result<(), String> {
        debug!("#DBG: Releasing TestTask");
        Ok(())
    }
}

// Tests multiple calls to TaskProcessor::init
#[tokio::test]
async fn test_multiple_init() {
    init_tracing();
    let _guard = TEST_LOCK.lock().await;

    // Initialize TaskProcessor
    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

    // Call init multiple times
    let processor1 = TaskProcessor::init();
    let processor2 = TaskProcessor::init();
    let processor3 = TaskProcessor::init();

    // Verify that all calls return the same instance
    assert!(Arc::ptr_eq(&processor1, &processor2), "Expected same TaskProcessor instance");
    assert!(Arc::ptr_eq(&processor2, &processor3), "Expected same TaskProcessor instance");

    // Add a task to ensure functionality
    let task = TestTask::new();
    let task_id = processor.add(Box::new(task)).await.expect("Failed to add task");

    // Wait for task to be processed
    debug!("#DBG: Waiting 1000ms for task to complete");
    processor.wait_completed().await;
    sleep(TokioDuration::from_millis(1000)).await;

    // Check if task is in completed queue
    let completed_tasks = processor.get_completed_tasks().await;
    assert_eq!(completed_tasks.len(), 1, "Expected exactly one completed task");

    // Check find_task by ID
    let found_task = processor.find_task(task_id).await;
    assert!(found_task.is_some(), "Expected to find task with id={}", task_id);
    if let Some(t) = found_task {
        let t_read = t.read().await;
        assert_eq!(t_read.id(), task_id, "Expected task ID to match via find_task");
        assert_eq!(t_read.result(), Value::String("Completed successfully".to_string()), "Expected task result to match via find_task");
    }

    // Clean up
    processor.remove(task_id).await.expect("Failed to remove task");

    info!("#INFO: Successfully tested multiple TaskProcessor::init calls");
}

// Tests TaskProcessor by adding a TestTask and checking its completion
#[tokio::test]
async fn test_task_processor_add_and_run() {
    init_tracing();
    let _guard = TEST_LOCK.lock().await;

    // Initialize TaskProcessor
    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

    // Print initial status
    processor.print_status().await;

    // Add task to scheduled queue
    debug!("#DBG: Adding TestTask to TaskProcessor");
    let task = TestTask::new();
    let task_id = processor.add(Box::new(task)).await.expect("Failed to add task");

    // Wait for task to be processed
    debug!("#DBG: Waiting 1000ms for task to complete");
    processor.wait_completed().await;
    sleep(TokioDuration::from_millis(1000)).await;

    // Print status after processing
    processor.print_status().await;

    // Check if task is in completed queue
    let completed_tasks = processor.get_completed_tasks().await;
    debug!("#DBG: Completed tasks count: {}", completed_tasks.len());
    let mut task_found = false;
    for (_, t) in completed_tasks.iter() {
        let t_read = t.read().await;
        let result = t_read.result() == Value::String("Completed successfully".to_string());
        let status = t_read.status() == TaskStatus::Completed;
        debug!("#DBG: Task in completed queue: id={}, result={:?}, status={:?}", t_read.id(), t_read.result(), t_read.status());
        if result && status {
            task_found = true;
            break;
        }
    }
    assert!(task_found, "Expected task to be in completed queue with correct result and status");
    assert_eq!(completed_tasks.len(), 1, "Expected exactly one completed task");

    // Check find_completed by ID
    let found_task = processor.find_completed(task_id).await;
    assert!(found_task.is_some(), "Expected to find task with id={}", task_id);
    if let Some(t) = found_task {
        let t_read = t.read().await;
        assert_eq!(t_read.id(), task_id, "Expected task ID to match");
        assert_eq!(t_read.result(), Value::String("Completed successfully".to_string()), "Expected task result to be 'Completed successfully'");
        assert_eq!(t_read.status(), TaskStatus::Completed, "Expected task status to be Completed");
    }

    // Check find_task by ID
    let found_task = processor.find_task(task_id).await;
    assert!(found_task.is_some(), "Expected to find task with id={} via find_task", task_id);
    if let Some(t) = found_task {
        let t_read = t.read().await;
        assert_eq!(t_read.id(), task_id, "Expected task ID to match via find_task");
        assert_eq!(t_read.result(), Value::String("Completed successfully".to_string()), "Expected task result to be 'Completed successfully' via find_task");
        assert_eq!(t_read.status(), TaskStatus::Completed, "Expected task status to be Completed via find_task");
    }

    // Test remove by task_id
    processor.remove(task_id).await.expect("Failed to remove task");
    let completed_tasks = processor.get_completed_tasks().await;
    assert!(completed_tasks.is_empty(), "Expected completed queue to be empty after remove");

    // Clean up
    processor.wait_completed().await;
    processor.reset().await;

    info!("#INFO: Successfully tested TaskProcessor with TestTask");
}

// Tests TradesAggregator with TaskProcessor
#[tokio::test]
async fn test_trades_aggregator() {
    init_tracing();
    let _guard = TEST_LOCK.lock().await;

    // Initialize TaskProcessor
    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

    // Create a test account and TradesCache
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

    // Import sample trades from CSV
    trades_cache.import_csv("sample_trades.csv".to_string())
        .expect("Failed to import sample_trades.csv");

    // Create a TradesAggregator
    let start_ts = DateTime::parse_from_rfc3339("2024-12-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end_ts = DateTime::parse_from_rfc3339("2025-01-28T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let aggregator = TradesAggregator::new(
        trades_cache.clone(),
        start_ts,
        end_ts,
        Duration::days(7),
        CalcMethod::Coarse,
        true,
        false,
    ).await;

    // Add task to TaskProcessor
    debug!("#DBG: Adding TradesAggregator to TaskProcessor");
    let task_id = processor.add(Box::new(aggregator)).await.expect("Failed to add task");

    // Wait for task to complete
    debug!("#DBG: Waiting 1000ms for task to complete");
    processor.wait_completed().await;
    sleep(TokioDuration::from_millis(1000)).await;

    // Check if task is in completed queue
    let completed_tasks = processor.get_completed_tasks().await;
    debug!("#DBG: Completed tasks count: {}", completed_tasks.len());
    assert!(!completed_tasks.is_empty(), "Expected at least one completed task");

    // Check result via get_results
    let result = processor.get_results(task_id).await.expect("Failed to get task result");
    debug!("#DBG: TradesAggregator result: {:?}", result);
    assert!(result.is_array(), "Expected result to be a JSON array");
    let trades = result.as_array().expect("Result should be an array");
    assert!(!trades.is_empty(), "Expected non-empty array of trades");

    // Load expected results
    let expected_results: Value = serde_json::from_str(
        &std::fs::read_to_string("expected_results.json").expect("Failed to read expected_results.json")
    ).expect("Failed to parse expected_results.json");
    let expected_trades = expected_results["weekly"].as_array().expect("Expected weekly results array");
    assert_eq!(trades.len(), expected_trades.len(), "Unexpected number of trades for weekly aggregation");
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
        assert_eq!(
            trade["price"].as_f64().unwrap() as f32,
            expected["price"].as_f64().unwrap() as f32,
            "Price mismatch for trade {}",
            trade["trade_no"].as_str().unwrap()
        );
        assert_eq!(
            trade["amount"].as_f64().unwrap() as f32,
            expected["amount"].as_f64().unwrap() as f32,
            "Amount mismatch for trade {}",
            trade["trade_no"].as_str().unwrap()
        );
        assert_eq!(
            trade["trade_no"].as_str().unwrap(),
            expected["trade_no"].as_str().unwrap(),
            "Trade_no mismatch for trade {}",
            trade["trade_no"].as_str().unwrap()
        );
    }

    // Check find_task by ID
    let found_task = processor.find_task(task_id).await;
    assert!(found_task.is_some(), "Expected to find task with id={} via find_task", task_id);
    if let Some(t) = found_task {
        let t_read = t.read().await;
        assert_eq!(t_read.id(), task_id, "Expected task ID to match via find_task");
        assert_eq!(t_read.result(), result, "Expected task result to match via find_task");
        assert_eq!(t_read.status(), TaskStatus::Completed, "Expected task status to be Completed via find_task");
    }

    // Test remove by task_id
    processor.remove(task_id).await.expect("Failed to remove task");
    let completed_tasks = processor.get_completed_tasks().await;
    assert!(completed_tasks.is_empty(), "Expected completed queue to be empty after remove");

    // Clean up
    processor.wait_completed().await;
    processor.reset().await;

    info!("#INFO: Successfully tested TradesAggregator with TaskProcessor");
}

// Tests TradesAggregator with Precise mode
#[tokio::test]
async fn test_trades_aggregator_precise() {
    init_tracing();
    let _guard = TEST_LOCK.lock().await;

    // Initialize TaskProcessor
    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

    // Create a test account and TradesCache
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

    // Import sample trades from CSV
    trades_cache.import_csv("sample_trades.csv".to_string())
        .expect("Failed to import sample_trades.csv");

    // Create a TradesAggregator
    let start_ts = DateTime::parse_from_rfc3339("2024-12-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let end_ts = DateTime::parse_from_rfc3339("2025-01-28T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let aggregator = TradesAggregator::new(
        trades_cache.clone(),
        start_ts,
        end_ts,
        Duration::days(7),
        CalcMethod::Precise,
        false,
        false,
    ).await;

    // Add task to TaskProcessor
    debug!("#DBG: Adding TradesAggregator to TaskProcessor");
    let task_id = processor.add(Box::new(aggregator)).await.expect("Failed to add task");

    // Wait for task to complete
    debug!("#DBG: Waiting 1000ms for task to complete");
    processor.wait_completed().await;
    sleep(TokioDuration::from_millis(1000)).await;

    // Check if task is in completed queue
    let completed_tasks = processor.get_completed_tasks().await;
    debug!("#DBG: Completed tasks count: {}", completed_tasks.len());
    assert!(!completed_tasks.is_empty(), "Expected at least one completed task");

    // Check result via get_results
    let result = processor.get_results(task_id).await.expect("Failed to get task result");
    debug!("#DBG: TradesAggregator precise result: {:?}", result);
    assert!(result.is_array(), "Expected result to be a JSON array");
    let trades = result.as_array().expect("Result should be an array");
    assert_eq!(trades.len(), 20, "Expected 20 trades for precise aggregation");

    // Load expected results
    let expected_results: Value = serde_json::from_str(
        &std::fs::read_to_string("expected_results.json").expect("Failed to read expected_results.json")
    ).expect("Failed to parse expected_results.json");
    let expected_trades = expected_results["precise"].as_array().expect("Expected precise results array");
    assert_eq!(trades.len(), expected_trades.len(), "Unexpected number of trades for precise aggregation");
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
        assert_eq!(
            trade["price"].as_f64().unwrap() as f32,
            expected["price"].as_f64().unwrap() as f32,
            "Price mismatch for trade {}",
            trade["trade_no"].as_str().unwrap()
        );
        assert_eq!(
            trade["amount"].as_f64().unwrap() as f32,
            expected["amount"].as_f64().unwrap() as f32,
            "Amount mismatch for trade {}",
            trade["trade_no"].as_str().unwrap()
        );
        assert_eq!(
            trade["trade_no"].as_str().unwrap(),
            expected["trade_no"].as_str().unwrap(),
            "Trade_no mismatch for trade {}",
            trade["trade_no"].as_str().unwrap()
        );
    }

    // Check find_task by ID
    let found_task = processor.find_task(task_id).await;
    assert!(found_task.is_some(), "Expected to find task with id={} via find_task", task_id);
    if let Some(t) = found_task {
        let t_read = t.read().await;
        assert_eq!(t_read.id(), task_id, "Expected task ID to match via find_task");
        assert_eq!(t_read.result(), result, "Expected task result to match via find_task");
        assert_eq!(t_read.status(), TaskStatus::Completed, "Expected task status to be Completed via find_task");
    }

    // Test remove by task_id
    processor.remove(task_id).await.expect("Failed to remove task");
    let completed_tasks = processor.get_completed_tasks().await;
    assert!(completed_tasks.is_empty(), "Expected completed queue to be empty after remove");

    // Clean up
    processor.wait_completed().await;
    processor.reset().await;

    info!("#INFO: Successfully tested TradesAggregator with Precise mode");
}