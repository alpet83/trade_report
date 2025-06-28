use chrono::{DateTime, Utc, Duration};
use dashmap::DashMap;
use once_cell::sync::OnceCell;
use std::sync::{Arc, atomic::{AtomicU32, Ordering, AtomicBool}, Mutex};
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration as TokioDuration};
use tracing::{info, debug, warn, error};
use serde_json::Value;

use crate::{
    entities::task::{Task, TaskStatus},
    logs::app_error::AppError,
};

// Default delay for polling tasks
const DEFAULT_DELAY: TokioDuration = TokioDuration::from_millis(50); // Возвращено к 50 мс

// Singleton instance for TaskProcessor
static TASK_PROCESSOR: OnceCell<Arc<TaskProcessor>> = OnceCell::new();

// Thread-safe task ID counter
static TASK_ID_COUNTER: OnceCell<Arc<AtomicU32>> = OnceCell::new();

type SharedTask = Arc<RwLock<Box<dyn Task + 'static>>>;

fn singleton_task_processor() -> Arc<TaskProcessor> {
    TASK_PROCESSOR.get().expect("#ERROR(singleton_task_processor): TaskProcessor not initialized").clone()
}

// Message type for the scheduled queue
#[derive(Debug)]
struct TaskScheduled {
    at: DateTime<Utc>,
    task: SharedTask,
}

// Guard to track background thread termination
struct ThreadGuard {
    thread_name: String,
    cycle_count: u32,
}

impl Drop for ThreadGuard {
    fn drop(&mut self) {
        warn!("#WARN: Thread {} terminated after {} cycles", self.thread_name, self.cycle_count);
    }
}

// Manages a background thread for processing tasks
#[derive(Debug)]
pub struct TaskProcessor {
    scheduled: Arc<DashMap<DateTime<Utc>, SharedTask>>,
    completed: Arc<RwLock<std::collections::HashMap<u32, TaskScheduled>>>,
    failed_count: Arc<RwLock<u32>>,
    thread_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    is_spawning: Arc<AtomicBool>,
    thread_active: Arc<AtomicBool>, // Добавлено
}

impl TaskProcessor {
    fn thread_need_respawn(&self) -> bool {
        let thread_handle = self.thread_handle.lock().unwrap();
        thread_handle.is_none() || thread_handle.as_ref().map_or(true, |handle| handle.is_finished())
    }

    // Checks if the background thread is running and spawns a new one if necessary
    fn check_spawn_thread(&self, context: &str) {
        if self.thread_need_respawn() {
            if self.is_spawning.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                let mut thread_handle = self.thread_handle.lock().unwrap();
                debug!("#WARN: Need respawn thread: {:?} at {}", thread_handle, context);
                let aps = singleton_task_processor();
                let handle = tokio::spawn(async move {
                    aps.run_background_thread().await;
                    warn!("#WARN: Background thread terminated");
                });
                info!("#INFO: Spawned new background thread, finished: {:?}", handle.is_finished());
                *thread_handle = Some(handle);
                self.thread_active.store(true, Ordering::SeqCst); // Устанавливаем thread_active=true
                self.is_spawning.store(false, Ordering::SeqCst);
            } else {
                debug!("#DBG: Skipped spawning thread: already spawning at {}", context);
            }
        }
    }

    // Creates a new TaskProcessor instance and starts the background thread
    pub fn new() -> Arc<Self> {
        TASK_ID_COUNTER.get_or_init(|| Arc::new(AtomicU32::new(1)));
        let processor = Arc::new(TaskProcessor {
            scheduled: Arc::new(DashMap::new()),
            completed: Arc::new(RwLock::new(std::collections::HashMap::new())),
            failed_count: Arc::new(RwLock::new(0)),
            thread_handle: Arc::new(Mutex::new(None)),
            is_spawning: Arc::new(AtomicBool::new(false)),
            thread_active: Arc::new(AtomicBool::new(true)), // Инициализируем как true
        });
        let processor_clone = processor.clone();
        let mut thread_handle = processor_clone.thread_handle.lock().unwrap();
        let processor_clone = processor.clone();
        let handle = tokio::spawn(async move {
            processor_clone.run_background_thread().await;
            warn!("#WARN: Background thread terminated");
        });
        *thread_handle = Some(handle);
        debug!("#DBG: Spawned thread: {:?}", thread_handle);
        info!("#INFO: Initialized TaskProcessor singleton");
        processor
    }

    // Retrieves the global TaskProcessor singleton
    pub fn get() -> Arc<Self> {
        let processor = singleton_task_processor();
        processor.check_spawn_thread("::get");
        processor
    }

    // Initializes the global TaskProcessor singleton
    pub fn init() -> Arc<Self> {
        let processor = TASK_PROCESSOR.get_or_init(|| Self::new()).clone();
        processor.check_spawn_thread("::init");
        processor
    }

    // Resets the TaskProcessor state for testing
    pub async fn reset(&self) {
        if !self.scheduled.is_empty() {
            warn!("#WARN: Resetting TaskProcessor with {} tasks in scheduled queue", self.scheduled.len());
        }
        self.scheduled.clear();
        let mut completed = self.completed.write().await;
        let removed_count = completed.len();
        for (_, task_scheduled) in completed.drain() {
            if let Err(e) = task_scheduled.task.write().await.release().await {
                error!("Failed to release task during reset: {}", e);
            }
        }
        let mut failed_count = self.failed_count.write().await;
        *failed_count = 0;
        if let Some(counter) = TASK_ID_COUNTER.get() {
            counter.store(1, Ordering::SeqCst);
        }
        self.thread_active.store(false, Ordering::SeqCst); // Сбрасываем thread_active
        info!("#INFO: TaskProcessor state reset: {} tasks removed from completed", removed_count);
    }

    // Returns the number of tasks still scheduled
    async fn still_scheduled(&self) -> usize {
        self.scheduled.len()
    }

    // Waits for all pending tasks to be processed
    pub async fn wait_completed(&self) {
        debug!("#SYNC: Waiting for {} pending tasks to complete", self.still_scheduled().await);
        tokio::time::timeout(TokioDuration::from_secs(15), async {
            while self.still_scheduled().await > 0 {
                sleep(TokioDuration::from_millis(10)).await;
            }
        }).await.unwrap_or_else(|_| debug!("#SYNC_WARN: Timeout waiting for pending tasks"));
        debug!("#SYNC: Finished waiting: {} pending tasks remain", self.still_scheduled().await);
    }

    // Finds a task by ID in any queue
    pub async fn find_task(&self, task_id: u32) -> Option<SharedTask> {
        debug!("#DBG: Fetching task with task_id={}", task_id);
        for entry in self.scheduled.iter() {
            if entry.value().read().await.id() == task_id {
                debug!("#DBG: Found task_id={} in scheduled queue", task_id);
                return Some(entry.value().clone());
            }
        }
        let completed = self.completed.read().await;
        for (_id, task_scheduled) in completed.iter() {                   
            debug!("#DBG: Found task_id={} in completed queue", task_id);
            return Some(task_scheduled.task.clone());            
        }
        debug!("#DBG: Task_id={} not found in any queue", task_id);
        None
    }

    // Processes tasks from the scheduled queue
    async fn process(&self) {
        let now = Utc::now();
        let tasks_to_run: Vec<(DateTime<Utc>, SharedTask)> = {
            let mut tasks = Vec::new();
            for entry in self.scheduled.iter() {
                if *entry.key() <= now {
                    tasks.push((*entry.key(), entry.value().clone()));
                }
            }
            tasks.sort_by(|a, b| a.0.cmp(&b.0));
            for (at, _) in &tasks {
                self.scheduled.remove(at);
            }
            tasks
        };

        debug!("#DBG: Processing {} tasks in current cycle", tasks_to_run.len());
        for (at, task) in &tasks_to_run {
            debug!("Executing task with at={}", at);
            let mut status = TaskStatus::Failed;
            let run_result = task.write().await.run().await;
            if let Ok(s) = run_result {
                status = s;
            } else {
                error!("#ERROR: Task execution failed: {:?}", run_result);
            }

            match status {
                TaskStatus::Completed => {
                    let task_id = task.read().await.id();
                    let mut completed = self.completed.write().await;
                    completed.insert(
                        task_id,
                        TaskScheduled {
                            at: *at,
                            task: task.clone(),
                        },
                    );
                    info!("Task {} completed, moved to completed queue (total: {})", task_id, completed.len());
                }
                TaskStatus::Failed => {
                    let mut failed_count = self.failed_count.write().await;
                    *failed_count += 1;
                    if let Err(e) = task.write().await.release().await {
                        error!("Failed to release task: {}", e);
                    }
                    info!("Task failed, released and discarded");
                }
                TaskStatus::New | TaskStatus::Scheduled => {
                    let task_id = task.read().await.id();
                    let new_at = task.read().await.start_at();
                    self.scheduled.insert(new_at, task.clone());
                    info!("Task {} rescheduled at {}", task_id, new_at);
                }
                TaskStatus::Postponed => {
                    let task_id = task.read().await.id();
                    let new_at = Utc::now() + Duration::milliseconds(100);
                    task.write().await.set_start_at(new_at);
                    self.scheduled.insert(new_at, task.clone());
                    info!("Task {} postponed, rescheduled at {}", task_id, new_at);
                }
            }
            debug!("#DBG: Completed processing task at={}, {} tasks remaining in cycle", at, tasks_to_run.len() - (tasks_to_run.iter().position(|(t, _)| t == at).unwrap() + 1));
        }

        let scheduled_len = self.still_scheduled().await;
        debug!("#DBG: End of loop: {} tasks in scheduled", scheduled_len);
    }

    // Runs the background thread to process tasks
    async fn run_background_thread(&self) {
        info!("#INFO: >>>>>>>>>> Starting TaskProcessor background thread <<<<<<<<<<<<");
        let mut guard = ThreadGuard { thread_name: "TaskProcessor".to_string(), cycle_count: 0 };
        loop {
            self.process().await;
            guard.cycle_count += 1;
            if self.scheduled.is_empty() && !self.thread_active.load(Ordering::SeqCst) {
                debug!("#DBG: No tasks in scheduled queue and thread_active=false, pausing background thread");
                break;
            }
            sleep(DEFAULT_DELAY).await;
        }
    }

    // Adds a task to the scheduled queue
    pub async fn add(&self, task: Box<dyn Task + 'static>) -> Result<u32, String> {
        debug!("Adding task to TaskProcessor");
        let task_arc = Arc::new(RwLock::new(task));
        let mut task_write = task_arc.write().await;
        task_write.init().await?;
        task_write.set_status(TaskStatus::Scheduled);
        let at = Utc::now() + Duration::milliseconds(50);
        let task_id = TASK_ID_COUNTER
            .get()
            .expect("Task ID counter not initialized")
            .fetch_add(1, Ordering::SeqCst);
        task_write.set_id(task_id);
        task_write.set_start_at(at);
        drop(task_write);

        self.replay_at(at, task_arc.clone(), task_id).await?;
        info!("Task {} scheduled at {}", task_id, at);
        Ok(task_id)
    }

    // Replays a task at a new time
    pub async fn replay_at(&self, at: DateTime<Utc>, task: SharedTask, task_id: u32) -> Result<(), String> {
        let mut task_write = task.write().await;
        task_write.set_start_at(at);
        task_write.set_status(TaskStatus::Scheduled);
        task_write.set_id(task_id);
        drop(task_write);

        self.scheduled.insert(at, task.clone());
        Ok(())
    }

    // Removes a task from the completed queue by task_id
    pub async fn remove(&self, task_id: u32) -> Result<(), String> {
        let mut completed = self.completed.write().await;
        if let Some(task_scheduled) = completed.remove(&task_id) {
            task_scheduled.task.write().await.release().await?;
            info!("Task {} removed from completed queue", task_id);
            Ok(())
        } else {
            Err(format!("Task with id={} not found", task_id))
        }
    }

    // Finds a completed task by ID
    pub async fn find_completed(&self, id: u32) -> Option<SharedTask> {
        let completed = self.completed.read().await;
        completed.get(&id).map(|task_scheduled| task_scheduled.task.clone())
    }

    // Retrieves completed tasks
    pub async fn get_completed_tasks(&self) -> Vec<(DateTime<Utc>, SharedTask)> {
        let completed = self.completed.read().await;
        completed
            .iter()
            .map(|(_, task_scheduled)| (task_scheduled.at, task_scheduled.task.clone()))
            .collect()
    }

    // Retrieves the result of a completed task by ID
    pub async fn get_results(&self, id: u32) -> Result<Value, AppError> {
        debug!("Fetching result for task_id={}", id);
        let task = self
            .find_completed(id)
            .await
            .ok_or_else(|| AppError::NotFound(format!("Task with id={} not found", id)))?;

        let task_read = task.read().await;
        let strong_count = Arc::strong_count(&task);
        let task_result = task_read.result();
        debug!("Retrieved result for task_id={} (strong_count={}): {:?}", id, strong_count, task_result);
        Ok(task_result)
    }

    // Prints the status of completed and failed tasks
    pub async fn print_status(&self) {
        let completed = self.completed.read().await;
        let completed_count = completed.len();
        let failed_count = *self.failed_count.read().await;
        let pending_count = self.still_scheduled().await;
        info!(
            "#STATUS: TaskProcessor status: {} completed, {} failed, {} pending",
            completed_count, failed_count, pending_count
        );
    }
}