use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration as TokioDuration};
use tracing::{info, debug};
use tracing_subscriber::EnvFilter;
use delegate::delegate;
use std::sync::Arc;

use crate::{
    entities::task::{TaskStatus, Task, TaskBase},
    services::task_processor::TaskProcessor,
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

    async fn init(&mut self) -> Result<(), String> {
        debug!("#DBG: Initializing TestTask");
        Ok(())
    }

    async fn run(&mut self) -> Result<TaskStatus, String> {
        debug!("#DBG: Running TestTask");
        self.set_result(Value::String("Completed successfully".to_string()));
        self.set_status(TaskStatus::Completed);
        Ok(TaskStatus::Completed)
    }

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

    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

    let processor1 = TaskProcessor::init();
    let processor2 = TaskProcessor::init();
    let processor3 = TaskProcessor::init();

    assert!(Arc::ptr_eq(&processor1, &processor2), "Expected same TaskProcessor instance");
    assert!(Arc::ptr_eq(&processor2, &processor3), "Expected same TaskProcessor instance");

    let task = TestTask::new();
    let task_id = processor.add(Box::new(task)).await.expect("Failed to add task");

    debug!("#DBG: Waiting 1000ms for task to complete");
    processor.wait_completed().await;
    sleep(TokioDuration::from_millis(1000)).await;

    let completed_tasks = processor.get_completed_tasks().await;
    assert_eq!(completed_tasks.len(), 1, "Expected exactly one completed task");

    let found_task = processor.find_task(task_id).await;
    assert!(found_task.is_some(), "Expected to find task with id={}", task_id);
    if let Some(t) = found_task {
        let t_read = t.read().await;
        assert_eq!(t_read.id(), task_id, "Expected task ID to match");
        assert_eq!(t_read.result(), Value::String("Completed successfully".to_string()), "Expected task result to match");
    }

    processor.remove(task_id).await.expect("Failed to remove task");

    info!("#INFO: Successfully tested multiple TaskProcessor::init calls");
}

// Tests TaskProcessor by adding a TestTask and checking its completion
#[tokio::test]
async fn test_task_processor_add_and_run() {
    init_tracing();
    let _guard = TEST_LOCK.lock().await;

    let processor = TaskProcessor::init();
    processor.wait_completed().await;
    processor.reset().await;

    processor.print_status().await;

    debug!("#DBG: Adding TestTask to TaskProcessor");
    let task = TestTask::new();
    let task_id = processor.add(Box::new(task)).await.expect("Failed to add task");

    debug!("#DBG: Waiting 1000ms for task to complete");
    processor.wait_completed().await;
    sleep(TokioDuration::from_millis(1000)).await;

    processor.print_status().await;

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

    let found_task = processor.find_completed(task_id).await;
    assert!(found_task.is_some(), "Expected to find task with id={}", task_id);
    if let Some(t) = found_task {
        let t_read = t.read().await;
        assert_eq!(t_read.id(), task_id, "Expected task ID to match");
        assert_eq!(t_read.result(), Value::String("Completed successfully".to_string()), "Expected task result to be 'Completed successfully'");
        assert_eq!(t_read.status(), TaskStatus::Completed, "Expected task status to be Completed");
    }

    let found_task = processor.find_task(task_id).await;
    assert!(found_task.is_some(), "Expected to find task with id={} via find_task", task_id);
    if let Some(t) = found_task {
        let t_read = t.read().await;
        assert_eq!(t_read.id(), task_id, "Expected task ID to match via find_task");
        assert_eq!(t_read.result(), Value::String("Completed successfully".to_string()), "Expected task result to be 'Completed successfully' via find_task");
        assert_eq!(t_read.status(), TaskStatus::Completed, "Expected task status to be Completed via find_task");
    }

    processor.remove(task_id).await.expect("Failed to remove task");
    let completed_tasks = processor.get_completed_tasks().await;
    assert!(completed_tasks.is_empty(), "Expected completed queue to be empty after remove");

    processor.wait_completed().await;
    processor.reset().await;

    info!("#INFO: Successfully tested TaskProcessor with TestTask");
}