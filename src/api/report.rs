use axum::{Router, routing::get, Json};
use crate::services::report::Report;

pub fn routes() -> Router {
    Router::new().route("/report", get(get_report))
}

async fn get_report() -> Json<Report> {
    // TODO: Implement report fetching logic
    Json(Report {
        account_id: 0,
        start: chrono::Utc::now(),
        end: chrono::Utc::now(),
        total_pnl: 0.0,
        trade_count: 0,
    })
}