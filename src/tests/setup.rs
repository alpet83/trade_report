use once_cell::sync::OnceCell;
use std::fs;
use tracing::debug;

use crate::common::basic_logger::{logger};

static LOGS_CLEANUP: OnceCell<()> = OnceCell::new();

pub async fn init_test_environment() {
    debug!("#DBG: Starting init_test_environment");
    LOGS_CLEANUP.get_or_init(|| {
        let test_log_dir = "test-logs";
        logger::set_log_dir(test_log_dir);
        debug!("#DBG: Set log directory to {}", test_log_dir);
        if fs::metadata(test_log_dir).is_ok() {
            debug!("#DBG: Cleaning up test log directory {}", test_log_dir);
            fs::remove_dir_all(test_log_dir).expect("Failed to clean up test log directory");
        }
        fs::create_dir_all(test_log_dir).expect("Failed to create test log directory");
        debug!("#DBG: Test log directory {} initialized", test_log_dir);
    });
    debug!("#DBG: Finished init_test_environment");
}