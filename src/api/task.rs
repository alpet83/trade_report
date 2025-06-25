use axum::{Router, routing::get, Json};
use serde_json::Value;
use tracing::{info, error, debug};
use tokio::time::{timeout, Duration};
use backtrace::Backtrace;

use crate::{
    services::task_processor::TaskProcessor,
    logs::app_error::AppError,
};

// Configures API routes for task-related endpoints
pub fn routes() -> Router<()> {
    Router::new()
        .route("/completed", get(get_completed_tasks))
        .route("/results", get(get_task_results))
}

// Fetches the list of task IDs for completed tasks
async fn get_completed_tasks() -> Result<Json<Vec<u32>>, AppError> {
    info!("Starting request to fetch completed task IDs");

    let result = timeout(Duration::from_secs(10), async {
        debug!("Acquiring TaskProcessor singleton");
        let processor = TaskProcessor::get();
        let completed_tasks = processor.get_completed_tasks().await;
        let mut task_ids: Vec<u32> = Vec::new();
        for (_, task) in completed_tasks {
            let task_read = task.read().await;
            task_ids.push(task_read.id());
        }
        debug!("Retrieved {} completed task IDs", task_ids.len());
        Ok(Json(task_ids))
    })
    .await;

    match result {
        Ok(Ok(json)) => {
            info!("Completed task IDs request succeeded: {} tasks", json.0.len());
            Ok(json)
        }
        Ok(Err(e)) => {
            error!("Completed task IDs request failed: {:?}", e);
            Err(e)
        }
        Err(_) => {
            let backtrace = Backtrace::new();
            error!("Request timed out after 10 seconds\nBacktrace:\n{:?}", backtrace);
            Err(AppError::Internal(format!("Request timed out after 10 seconds\nBacktrace:\n{:?}", backtrace)))
        }
    }
}

// Fetches the result of a specific task by task_id
#[axum::debug_handler]
async fn get_task_results(
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, AppError> {
    info!("Starting request to fetch task result");

    let task_id = params
        .get("task_id")
        .ok_or_else(|| AppError::BadRequest("Missing task_id parameter".to_string()))?
        .parse::<u32>()
        .map_err(|e| AppError::BadRequest(format!("Invalid task_id: {}", e)))?;

    let result = timeout(Duration::from_secs(10), async {
        debug!("Calling TaskProcessor::get_results for task_id={}", task_id);
        let processor = TaskProcessor::get();
        processor.get_results(task_id).await
    })
    .await;

    match result {
        Ok(Ok(json)) => {
            info!("Task result request succeeded for task_id={}", task_id);
            Ok(Json(json))
        }
        Ok(Err(e)) => {
            error!("Task result request failed: {:?}", e);
            Err(e)
        }
        Err(_) => {
            let backtrace = Backtrace::new();
            error!("Request timed out after 10 seconds\nBacktrace:\n{:?}", backtrace);
            Err(AppError::Internal(format!("Request timed out after 10 seconds\nBacktrace:\n{:?}", backtrace)))
        }
    }
}