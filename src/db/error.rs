// Modified: 2025-06-19 08:35:00 EEST
// xaiArtifact: artifact_id="9723c877-e3db-4a13-b5fb-415d2508e70f", artifact_version_id="5e6f7a8b-9c0d-4e1f-2a3b-4c5d6e7f8a9b"

use sqlx::Error;
use tracing::error;
use backtrace::Backtrace;

pub fn handle_sql_error(query: &str, error: Error) -> String {
    let error_message = format!(
        "SQL error: {}\nQuery: {}\nBacktrace: {:?}",
        error, query, Backtrace::new()
    );
    error!("{}", error_message);
    error_message
}