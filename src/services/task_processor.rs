// /src/services/task_processor.rs
// Modified: 2025-06-23 16:45:00 EEST

use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration};
use dashmap::DashMap;
use once_cell::sync::OnceCell;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration as TokioDuration};
use tracing::{info, debug, error};

use crate::entities::task::{Task, Status};

// Singleton instance for TaskProcessor
static TASK_PROCESSOR: OnceCell<Arc<TaskProcessor>> = OnceCell::new();

// Manages a background thread for processing tasks
#[derive(Debug)]
pub struct TaskProcessor {
    scheduled: DashMap<DateTime<Utc>, Arc<RwLock<Box<dyn Task>>>>,
    completed: DashMap<DateTime<Utc>, Arc<RwLock<Box<dyn Task>>>>,
    failed_count: Arc<RwLock<u32>>,
}

// Detailed logic of TaskProcessor operation
// 1. Main thread adds tasks to the `scheduled` queue using `add`, which sets start_at to now + 50ms and calls `Task::init`.
// 2. Background thread checks `scheduled` every 50ms, extracting tasks where `start_at <= now`.
// 3. For each extracted task:
//    - Calls `Task::run` to execute the task.
//    - Based on the returned `Status`:
//      - `Completed`: Moves task to `completed` queue with completion time as key.
//      - `Failed`: Increments `failed_count`, calls `Task::release`, and discards the task.
//      - `Postponed`: Returns task to `scheduled` queue with `start_at` as key.
//      - `New` or `Scheduled`: Invalid states, logs error, increments `failed_count`, and discards task.
// 4. Background thread cleans up `completed` queue, removing tasks older than 10 minutes with `release`.
// 5. Methods `add`, `remove`, `replay_at` ensure thread-safe queue management.
// 6. Tasks are wrapped in `Arc<RwLock<Box<dyn Task>>>` for thread safety.
// 7. Method `print_status` logs the number of scheduled, completed, and failed tasks.

impl TaskProcessor {
    // Creates a new TaskProcessor instance and starts the background thread
    pub fn new() -> Arc<Self> {
        let processor = Arc::new(TaskProcessor {
            scheduled: DashMap::new(),
            completed: DashMap::new(),
            failed_count: Arc::new(RwLock::new(0)),
        });

        let processor_clone = processor.clone();
        tokio::spawn(async move {
            processor_clone.run_background_thread().await;
        });

        info!("Initialized TaskProcessor singleton");
        processor
    }

    // Retrieves the global TaskProcessor singleton
    pub fn get() -> Arc<Self> {
        TASK_PROCESSOR
            .get()
            .expect("TaskProcessor not initialized")
            .clone()
    }

    // Initializes the global TaskProcessor singleton
    pub fn init() -> Arc<Self> {
        let processor = Self::new();
        TASK_PROCESSOR
            .set(processor.clone())
            .expect("TaskProcessor already initialized");
        processor
    }

    // Runs the background thread to process tasks and clean up completed tasks
    async fn run_background_thread(&self) {
        info!("Starting TaskProcessor background thread");
        loop {
            let now = Utc::now();

            // Process scheduled tasks
            let tasks_to_run: Vec<(DateTime<Utc>, Arc<RwLock<Box<dyn Task>>>)> = self
                .scheduled
                .iter()
                .filter(|entry| *entry.key() <= now)
                .map(|entry| (*entry.key(), entry.value().clone()))
                .collect();

            for (start_at, task) in tasks_to_run {
                // Remove task from scheduled queue
                self.scheduled.remove(&start_at);

                debug!("Executing task with start_at={}", start_at);
                let status = match task.write().await.run().await {
                    Ok(status) => status,
                    Err(e) => {
                        error!("Task execution failed: {}", e);
                        Status::Failed
                    }
                };

                match status {
                    Status::Completed => {
                        let completion_time = Utc::now();
                        self.completed.insert(completion_time, task.clone());
                        info!("Task completed at {}, moved to completed queue", completion_time);
                    }
                    Status::Failed => {
                        {
                            let mut failed_count = self.failed_count.write().await;
                            *failed_count += 1;
                        }
                        if let Err(e) = task.write().await.release().await {
                            error!("Failed to release task: {}", e);
                        }
                        info!("Task failed, released and discarded");
                    }
                    Status::Postponed => {
                        let new_start_at = task.read().await.start_at();
                        self.scheduled.insert(new_start_at, task.clone());
                        info!("Task postponed, rescheduled at {}", new_start_at);
                    }
                    _ => {
                        error!("Invalid task status after run: {:?}", status);
                        {
                            let mut failed_count = self.failed_count.write().await;
                            *failed_count += 1;
                        }
                        if let Err(e) = task.write().await.release().await {
                            error!("Failed to release task: {}", e);
                        }
                    }
                }
            }

            // Clean up completed tasks older than 10 minutes
            let cleanup_threshold = now - Duration::minutes(10);
            let tasks_to_cleanup: Vec<(DateTime<Utc>, Arc<RwLock<Box<dyn Task>>>)> = self
                .completed
                .iter()
                .filter(|entry| *entry.key() <= cleanup_threshold)
                .map(|entry| (*entry.key(), entry.value().clone()))
                .collect();

            for (completion_time, task) in tasks_to_cleanup {
                self.completed.remove(&completion_time);
                if let Err(e) = task.write().await.release().await {
                    error!("Failed to release completed task at {}: {}", completion_time, e);
                }
                info!("Cleaned up completed task from {}", completion_time);
            }

            // Sleep to avoid busy loop
            sleep(TokioDuration::from_millis(50)).await;
        }
    }

    // Adds a task to the scheduled queue and initializes it
    pub async fn add(&self, task: Box<dyn Task>) -> Result<(), String> {
        debug!("Adding task to TaskProcessor");
        let task_arc = Arc::new(RwLock::new(task));
        let mut task_write = task_arc.write().await;
        task_write.init().await?;
        task_write.set_status(Status::Scheduled);
        let start_at = Utc::now() + Duration::milliseconds(50);
        task_write.set_start_at(start_at);
        drop(task_write);
        self.scheduled.insert(start_at, task_arc);
        info!("Task scheduled at {}", start_at);
        Ok(())
    }

    // Replays a task at a new start time (used internally by add)
    pub async fn replay_at(
        &self,
        old_start_at: DateTime<Utc>,
        new_start_at: DateTime<Utc>,
        task: Arc<RwLock<Box<dyn Task>>>,
    ) -> Result<(), String> {
        // Remove task if it exists in scheduled queue
        if old_start_at != new_start_at {
            self.scheduled.remove(&old_start_at);
        }
        task.write().await.set_start_at(new_start_at);
        task.write().await.set_status(Status::Scheduled);
        self.scheduled.insert(new_start_at, task);
        info!("Task scheduled at {}", new_start_at);
        Ok(())
    }

    // Removes a task from the scheduled or completed queue
    pub async fn remove(&self, start_at: DateTime<Utc>) -> Result<(), String> {
        if let Some((_, task)) = self.scheduled.remove(&start_at) {
            task.write().await.release().await?;
            info!("Task removed from scheduled queue with start_at={}", start_at);
            Ok(())
        } else if let Some((_, task)) = self.completed.remove(&start_at) {
            task.write().await.release().await?;
            info!("Task removed from completed queue with start_at={}", start_at);
            Ok(())
        } else {
            Err(format!("Task not found with start_at={}", start_at))
        }
    }

    // Retrieves completed tasks for testing
    pub fn get_completed_tasks(&self) -> Vec<(DateTime<Utc>, Arc<RwLock<Box<dyn Task>>>)> {
        self.completed
            .iter()
            .map(|entry| (*entry.key(), entry.value().clone()))
            .collect()
    }

    // Prints the status of scheduled, completed, and failed tasks
    pub async fn print_status(&self) {
        let scheduled_count = self.scheduled.len();
        let completed_count = self.completed.len();
        let failed_count = *self.failed_count.read().await;
        info!(
            "TaskProcessor status: {} scheduled, {} completed, {} failed",
            scheduled_count, completed_count, failed_count
        );
    }
}