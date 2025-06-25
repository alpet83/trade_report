use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::time::{sleep, Duration};
use tracing::{info, debug};
use tracing_subscriber::EnvFilter;
use delegate::delegate;

use crate::{
    entities::task::{TaskStatus, Task, TaskBase},
    services::task_processor::TaskProcessor,
};

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
        debug!("Initializing TestTask");
        Ok(())
    }

    // Executes the task, always returning Completed
    async fn run(&mut self) -> Result<TaskStatus, String> {
        debug!("Running TestTask");
        self.set_result(Value::String("Completed successfully".to_string()));
        self.set_status(TaskStatus::Completed);
        Ok(TaskStatus::Completed)
    }

    // Releases resources (no-op for test)
    async fn release(&mut self) -> Result<(), String> {
        debug!("Releasing TestTask");
        Ok(())
    }
}

// Tests TaskProcessor by adding a TestTask and checking its completion
#[tokio::test]
async fn test_task_processor_add_and_run() {
    init_tracing();

    // Initialize TaskProcessor
    let processor = TaskProcessor::init();

    // Print initial status
    processor.print_status().await;

    // Add task to scheduled queue
    debug!("Adding TestTask to TaskProcessor");
    let task = TestTask::new();
    let task_id = processor.add(Box::new(task)).await.expect("Failed to add task");

    // Wait for task to be processed
    debug!("Waiting 150ms for task to complete");
    sleep(Duration::from_millis(150)).await;

    // Print status after processing
    processor.print_status().await;

    // Check if task is in completed queue
    let completed_tasks = processor.get_completed_tasks().await;
    debug!("Completed tasks count: {}", completed_tasks.len());
    let mut task_found = false;
    for (_, t) in completed_tasks.iter() {
        let t_read = t.read().await;
        let result = t_read.result() == Value::String("Completed successfully".to_string());
        let status = t_read.status() == TaskStatus::Completed;
        debug!("Task in completed queue: id={}, result={:?}, status={:?}", t_read.id(), t_read.result(), t_read.status());
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

    info!("Successfully tested TaskProcessor with TestTask");
}

/*
 * Note: Memory cleanup can be tested by creating a TradesAggregator with a large Vec<Trade>,
 * adding it to TaskProcessor, and checking Arc::strong_count and logs after release.
 * Use Task Manager or heaptrack to monitor memory usage.
 */