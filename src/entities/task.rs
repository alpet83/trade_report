// /src/entities/task.rs
// Modified: 2025-06-24 07:18:00 EEST

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::fmt::Debug;
use tracing::{debug};

use crate::services::task_processor::TaskProcessor;

// Defines the status of a task
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    New,
    Postponed,
    Scheduled,
    Completed,
    Failed,
}

// Defines the interface for tasks that can be processed by TaskProcessor
#[async_trait]
pub trait Task: Send + Sync + Debug {
    // Initializes the task, preparing it for execution
    async fn init(&mut self) -> Result<(), String>;
    // Executes the task, returning its status
    async fn run(&mut self) -> Result<TaskStatus, String>;
    // Releases resources associated with the task
    async fn release(&mut self) -> Result<(), String>;
    // Returns the task's status
    fn status(&self) -> TaskStatus;
    // Sets the task's status
    fn set_status(&mut self, status: TaskStatus);
    // Returns the task's result
    fn result(&self) -> Value;
    // Sets the task's result
    fn set_result(&mut self, result: Value);
    // Returns the task's start time
    fn start_at(&self) -> DateTime<Utc>;
    // Sets the task's start time
    fn set_start_at(&mut self, start_at: DateTime<Utc>);
    // Sets the task's ID
    fn set_id(&mut self, id: u32);
    // Returns the task's ID
    fn id(&self) -> u32;
}

// Base structure for common task fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskBase {
    id: u32,
    status: TaskStatus,
    result: Value,
    start_at: DateTime<Utc>,
}

impl TaskBase {
    // Creates a new TaskBase instance with start_at set to current time
    pub fn new() -> Self {
        TaskBase {
            id: 0, // ID will be set in TaskProcessor::replay_at
            status: TaskStatus::New,
            result: Value::Null,
            start_at: Utc::now(),
        }
    }

    // Returns the task's status
    pub fn status(&self) -> TaskStatus {
        self.status.clone()
    }

    // Sets the task's status
    pub fn set_status(&mut self, status: TaskStatus) {
        self.status = status;
    }

    // Returns the task's result
    pub fn result(&self) -> Value {
        self.result.clone()
    }

    // Sets the task's result
    pub fn set_result(&mut self, result: Value) {
        self.result = result;
    }

    // Returns the task's start time
    pub fn start_at(&self) -> DateTime<Utc> {
        self.start_at
    }

    // Sets the task's start time
    pub fn set_start_at(&mut self, start_at: DateTime<Utc>) {
        self.start_at = start_at;
    }

    // Returns the task's ID
    pub fn id(&self) -> u32 {
        self.id
    }

    // Sets the task's ID
    pub fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    // Registers the task in the TaskProcessor
    pub async fn self_reg<T: Task + Clone + 'static>(&self, task: T) -> Result<u32, String> {
        debug!("Registering task in TaskProcessor");
        let processor = TaskProcessor::get();
        processor.add(Box::new(task)).await
    }
}