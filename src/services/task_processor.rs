use chrono::{DateTime, Utc, Duration};
use dashmap::DashMap;
use once_cell::sync::OnceCell;
use std::{sync::{atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering}, Arc, Mutex}, thread::sleep as sync_sleep, time::Duration as TimeDuration};
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration as TokioDuration};
use tracing::{info, debug, warn};
use serde_json::Value;

use crate::{
    entities::task::{Task, TaskStatus},
    logs::app_error::AppError,
};

// Default delay for polling tasks
const DEFAULT_DELAY: TokioDuration = TokioDuration::from_millis(50);

// Atomic counter for active threads
static ACTIVE_THREADS: AtomicUsize = AtomicUsize::new(0);

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
        ACTIVE_THREADS.fetch_sub(1, Ordering::Release);
        debug!("#DBG: Thread {} dropped, active threads: {}", self.thread_name, ACTIVE_THREADS.load(Ordering::Acquire));
        warn!("#WARN: !!!!!!!!!!!!!!!!!!!! Thread {} terminated after {} cycles !!!!!!!!!!!!!!!!!!!!!", self.thread_name, self.cycle_count);
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
    thread_active: Arc<AtomicBool>,    
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
                                
                debug!("#DBG: Need respawn thread at {}", context);
                let aps = singleton_task_processor();

                while ACTIVE_THREADS.load(Ordering::Acquire) > 0 {                   
                    debug!("#DBG: Waiting for thread to finish at {}", context);
                    sync_sleep(TimeDuration::from_millis(100));                    
                }

                Self::spawn_thread(aps);               
                self.thread_active.store(true, Ordering::SeqCst); // prevent spawn additional threads
                self.is_spawning.store(false, Ordering::SeqCst);
            } else {
                debug!("#DBG: Skipped spawning: already in progress at {}", context);
            }
        }
    }

    fn spawn_thread(processor: Arc<Self>) {
        let processor_clone = processor.clone();
        let mut thread_handle = processor_clone.thread_handle.lock().unwrap();
        let processor_clone = processor.clone();
        ACTIVE_THREADS.fetch_add(1, Ordering::Release);
        debug!("#DBG: Active threads: {}", ACTIVE_THREADS.load(Ordering::Acquire));
        let handle = tokio::spawn({
            let processor_clone = processor_clone.clone();
            async move {
                processor_clone.run_background_thread().await;
                warn!("#WARN: Background thread terminated");
            }
        });
        *thread_handle = Some(handle);        
        sync_sleep(TimeDuration::from_millis(100)); // дождаться создания guard 
        debug!("#DBG: Spawned thread: {:?}", thread_handle);
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
            thread_active: Arc::new(AtomicBool::new(true)),
        });
        Self::spawn_thread(processor.clone());
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
            warn!("#WARN: Resetting with {} tasks in queue", self.scheduled.len());
        }
        self.scheduled.clear();
        let mut completed = self.completed.write().await;
        let removed_count = completed.len();
        for (_, task_scheduled) in completed.drain() {
            if let Err(e) = task_scheduled.task.write().await.release().await {
                warn!("#WARN: Failed to release task: {}", e);
            }
        }
        let mut failed_count = self.failed_count.write().await;
        *failed_count = 0;
        if let Some(counter) = TASK_ID_COUNTER.get() {
            counter.store(1, Ordering::SeqCst);
        }
        self.thread_active.store(false, Ordering::SeqCst);
        debug!("#DBG: Thread active set to false");
        info!("#INFO: TaskProcessor reset: {} tasks removed", removed_count);
    }

    // Returns the number of tasks still scheduled
    async fn still_scheduled(&self) -> usize {
        self.scheduled.len()
    }

    // Waits for all pending tasks to be processed
    pub async fn wait_completed(&self) {        
        let mut tasks = self.still_scheduled().await;    
        if  0 == tasks { return; }
        debug!("#DBG: Waiting for {} pending tasks", tasks);
        tokio::time::timeout(TokioDuration::from_secs(15), async {
            while tasks > 0 {
                self.check_spawn_thread("::wait_completed");
                sleep(TokioDuration::from_millis(30)).await;
                tasks = self.still_scheduled().await;
            }
        }).await.unwrap_or_else(|_| warn!("#SYNC_WARN: Timeout waiting for tasks"));
        
        if tasks > 0 {
            debug!("#DBG: Wait completed timeouted: {} tasks remain", tasks);
        }
        else {
            info!("#INFO: All tasks completed");
        }        
    }

    // Finds a task by ID in any queue
    pub async fn find_task(&self, task_id: u32) -> Option<SharedTask> {
        debug!("#DBG: Fetching task id={}", task_id);
        for entry in self.scheduled.iter() {
            if entry.value().read().await.id() == task_id {
                debug!("#DBG: Found task id={} in scheduled", task_id);
                return Some(entry.value().clone());
            }
        }
        let completed = self.completed.read().await;
        for (id, task_scheduled) in completed.iter() {
            if *id == task_id {
                debug!("#DBG: Found task id={} in completed", task_id);
                return Some(task_scheduled.task.clone());
            }
        }
        debug!("#DBG: Task id={} not found", task_id);
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

        let tasks = tasks_to_run.len();
        if tasks == 0 {
            return;
        }

        debug!("#DBG: Processing {} tasks", tasks);
        for (at, task) in &tasks_to_run {
            debug!("#DBG: Executing task at={}", at);
            let mut status = TaskStatus::Failed;
            let run_result = task.write().await.run().await;
            if let Ok(s) = run_result {
                status = s;
            } else {
                warn!("#ERROR: Task failed: {:?}", run_result);
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
                    info!("#INFO: Task {} completed (total: {})", task_id, completed.len());
                }
                TaskStatus::Failed => {
                    let mut failed_count = self.failed_count.write().await;
                    *failed_count += 1;
                    if let Err(e) = task.write().await.release().await {
                        warn!("#WARN: Failed to release task: {}", e);
                    }
                    info!("#INFO: Task failed and released");
                }
                TaskStatus::New | TaskStatus::Scheduled => {
                    let task_id = task.read().await.id();
                    let new_at = task.read().await.start_at();
                    self.scheduled.insert(new_at, task.clone());
                    info!("#INFO: Task {} rescheduled at {}", task_id, new_at);
                }
                TaskStatus::Postponed => {
                    let task_id = task.read().await.id();
                    let new_at = Utc::now() + Duration::milliseconds(100);
                    task.write().await.set_start_at(new_at);
                    self.scheduled.insert(new_at, task.clone());
                    info!("#INFO: Task {} postponed to {}", task_id, new_at);
                }
            }
            debug!("#DBG: Processed task at={}", at);
        }

        let scheduled_len = self.still_scheduled().await;
        debug!("#DBG: End of loop: {} tasks pending", scheduled_len);
    }

    // Runs the background thread to process tasks
    async fn run_background_thread(&self) {
        // let mut thread_handle = self.thread_handle.lock().unwrap();
        let thread_id = std::thread::current().id();
        info!("----------------------------------------------------------------------------------------------");
        info!("#INFO: >>>>>>>>>>>>>>>>>>>> Starting TaskProcessor thread:{:?} <<<<<<<<<<<<<<<<<<<", thread_id);
        let mut guard = ThreadGuard { thread_name: format!("TaskProcessor:{:?}", thread_id), cycle_count: 0 };
        loop {
            self.process().await;
            guard.cycle_count += 1;
            if self.scheduled.is_empty() && !self.thread_active.load(Ordering::SeqCst) {
                debug!("#DBG: No tasks and thread inactive, pausing thread:{}", guard.thread_name);
                break;
            }
            sleep(DEFAULT_DELAY).await;
        }
    }

    // Adds a task to the scheduled queue
    pub async fn add(&self, task: Box<dyn Task + 'static>) -> Result<u32, String> {
        debug!("#DBG: Adding task");
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
        info!("#INFO: Task {} scheduled at {}", task_id, at);
        self.check_spawn_thread("::add");
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
            info!("#INFO: Task {} removed", task_id);
            Ok(())
        } else {
            Err(format!("Task id={} not found", task_id))
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
        debug!("#DBG: Fetching result for task id={}", id);
        let task = self
            .find_completed(id)
            .await
            .ok_or_else(|| AppError::NotFound(format!("Task id={} not found", id)))?;

        let task_read = task.read().await;
        let strong_count = Arc::strong_count(&task);
        let task_result = task_read.result();
        debug!("#DBG: Got result for task id={} (refs={})", id, strong_count);
        Ok(task_result)
    }

    // Prints the status of completed and failed tasks
    pub async fn print_status(&self) {
        let completed = self.completed.read().await;
        let completed_count = completed.len();
        let failed_count = *self.failed_count.read().await;
        let pending_count = self.still_scheduled().await;
        info!(
            "#STATUS: TaskProcessor: {} completed, {} failed, {} pending",
            completed_count, failed_count, pending_count
        );
    }
}