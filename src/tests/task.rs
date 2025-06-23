// /src/tests/task.rs
// Modified: 2025-06-23 17:00:00 EEST

use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::time::{sleep, Duration};
use tracing::{info, debug};
use tracing_subscriber::EnvFilter;

use crate::{
    entities::task::{Status, Task, TaskBase},
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
    // Initializes the task (no-op for test)
    async fn init(&mut self) -> Result<(), String> {
        debug!("Initializing TestTask");
        Ok(())
    }

    // Executes the task, always returning Completed
    async fn run(&mut self) -> Result<Status, String> {
        debug!("Running TestTask");
        self.base.set_result(Value::String("Completed successfully".to_string()));
        self.base.set_status(Status::Completed);
        Ok(Status::Completed)
    }

    // Releases resources (no-op for test)
    async fn release(&mut self) -> Result<(), String> {
        debug!("Releasing TestTask");
        Ok(())
    }

    // Delegates status to TaskBase
    fn status(&self) -> Status {
        self.base.status()
    }

    // Delegates set_status to TaskBase
    fn set_status(&mut self, status: Status) {
        self.base.set_status(status);
    }

    // Delegates result to TaskBase
    fn result(&self) -> Value {
        self.base.result()
    }

    // Delegates set_result to TaskBase
    fn set_result(&mut self, result: Value) {
        self.base.set_result(result);
    }

    // Delegates start_at to TaskBase
    fn start_at(&self) -> DateTime<Utc> {
        self.base.start_at()
    }

    // Delegates set_start_at to TaskBase
    fn set_start_at(&mut self, start_at: DateTime<Utc>) {
        self.base.set_start_at(start_at);
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
    processor.add(Box::new(task)).await.expect("Failed to add task");

    // Wait for task to be processed
    debug!("Waiting 150ms for task to complete");
    sleep(Duration::from_millis(150)).await;

    // Print status after processing
    processor.print_status().await;

    // Check if task is in completed queue
    let completed_tasks = processor.get_completed_tasks();
    debug!("Completed tasks count: {}", completed_tasks.len());
    let mut task_found = false;
    for (_, t) in completed_tasks.iter() {
        let t_read = t.read().await;
        let result = t_read.result() == Value::String("Completed successfully".to_string());
        let status = t_read.status() == Status::Completed;
        debug!("Task in completed queue: result={:?}, status={:?}", t_read.result(), t_read.status());
        if result && status {
            task_found = true;
            break;
        }
    }
    assert!(task_found, "Expected task to be in completed queue with correct result and status");
    assert_eq!(completed_tasks.len(), 1, "Expected exactly one completed task");

    info!("Successfully tested TaskProcessor with TestTask");
}