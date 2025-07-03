use crate::common::basic_logger::logger;
use crate::tests::setup::init_test_environment;
use std::fs;
use std::path::Path;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration as TokioDuration, timeout};
use tracing::metadata::LevelFilter;
use tracing_subscriber::EnvFilter;
use regex::Regex;
use tracing::debug;

// Global mutex for test synchronization
static TEST_LOCK: Mutex<()> = Mutex::const_new(());

async fn init_tracing() {
    debug!("#DBG: Starting init_tracing");
    init_test_environment().await;
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::DEBUG.into())
                .from_env_lossy(),
        )
        .try_init();
    debug!("#DBG: Finished init_tracing");
}

fn get_log_prefix() -> String {
    std::env::current_exe()
        .map(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .map(|s| format!("{}#", s))
                .unwrap_or_else(|| "trade_report#".to_string())
        })
        .unwrap_or_else(|_| "trade_report#".to_string())
}

fn strip_ansi(content: &str) -> String {
    let re = Regex::new(r"\x1B\[(?:\d+(?:;\d+)*)?m").unwrap();
    re.replace_all(content, "").to_string()
}

async fn list_log_dir(log_dir: &str) {
    debug!("#DBG: Listing contents of log directory {}", log_dir);
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(log_dir) {
        for entry in entries.filter_map(Result::ok) {
            if let Ok(metadata) = entry.metadata() {
                let file_name = entry.file_name().to_string_lossy().into_owned();
                let size = metadata.len();
                let modified = metadata
                    .modified()
                    .map(|t| {
                        let dt: chrono::DateTime<chrono::Utc> = t.into();
                        dt.format("%Y-%m-%d %H:%M:%S").to_string()
                    })
                    .unwrap_or_default();
                files.push(format!("{} {} bytes, modified {}", file_name, size, modified));
            }
        }
    }
    logger::debug(&format!("#DBG: Contents of {}: {:?}", log_dir, files)).expect("Failed to log debug");
    debug!("#DBG: Finished listing log directory {}", log_dir);
}

#[tokio::test]
async fn test_basic_logger_multithread() {
    debug!("#DBG: Acquiring TEST_LOCK for test_basic_logger_multithread");
    let _guard = timeout(TokioDuration::from_secs(60), TEST_LOCK.lock())
        .await
        .expect("Timeout waiting for TEST_LOCK");
    debug!("#DBG: Acquired TEST_LOCK for test_basic_logger_multithread");

    init_tracing().await;

    let log_dir = "test-logs";
    fs::create_dir_all(log_dir).expect("Failed to create test log directory");
    logger::debug(&format!("#DBG: Created log directory {}", log_dir)).expect("Failed to log debug");

    let handles: Vec<_> = (0..3)
        .map(|i| {
            tokio::task::spawn_blocking(move || {
                logger::info(&format!("~C92Thread {} info message~C00", i))
                    .expect("Failed to log info");
                logger::warn(&format!("~C93Thread {} warning message~C00", i))
                    .expect("Failed to log warn");
                logger::debug(&format!("~C94Thread {} debug message~C00", i))
                    .expect("Failed to log debug");
                logger::error(&format!("~C91Thread {} error message~C00", i))
                    .expect("Failed to log error");
            })
        })
        .collect();

    for handle in handles {
        handle.await.expect("Thread panicked");
    }

    sleep(TokioDuration::from_millis(1000)).await;
    logger::debug("#DBG: Finished waiting for file writes in test_basic_logger_multithread")
        .expect("Failed to log debug");

    list_log_dir(log_dir).await;
    let thread_ids = logger::get_ids();
    logger::debug(&format!("#DBG: Retrieved thread IDs: {:?}", thread_ids))
        .expect("Failed to log debug");

    let mut all_info_found = vec![false; 3];
    let mut all_warn_found = vec![false; 3];
    let mut all_debug_found = vec![false; 3];
    let mut all_error_found = vec![false; 3];

    for thread_id in thread_ids.iter() {
        let log_file = format!("{}/{}{}.log", log_dir, get_log_prefix(), thread_id);
        logger::debug(&format!("#DBG: Checking log file {}", log_file))
            .expect("Failed to log debug");
        assert!(Path::new(&log_file).exists(), "Log file {} not found", log_file);
        let content = fs::read_to_string(&log_file).expect("Failed to read log file");
        let clean_content = strip_ansi(&content);
        logger::debug(&format!("#DBG: Log file {} contents:\n{}", log_file, clean_content))
            .expect("Failed to log debug");

        for i in 0..3 {
            if clean_content.contains(&format!(" [INFO] Thread {} info message", i)) {
                all_info_found[i] = true;
            }
            if clean_content.contains(&format!(" [WARN] Thread {} warning message", i)) {
                all_warn_found[i] = true;
            }
            if clean_content.contains(&format!(" [DEBUG] Thread {} debug message", i)) {
                all_debug_found[i] = true;
            }
            if clean_content.contains(&format!(" [ERROR] Thread {} error message\nBacktrace:", i)) {
                all_error_found[i] = true;
            }
        }
    }

    for i in 0..3 {
        assert!(
            all_info_found[i],
            "Expected '[INFO] Thread {} info message' in some log file",
            i
        );
        assert!(
            all_warn_found[i],
            "Expected '[WARN] Thread {} warning message' in some log file",
            i
        );
        assert!(
            all_debug_found[i],
            "Expected '[DEBUG] Thread {} debug message' in some log file",
            i
        );
        assert!(
            all_error_found[i],
            "Expected '[ERROR] Thread {} error message' in some log file",
            i
        );
    }

    list_log_dir(log_dir).await;
    logger::debug("#DBG: test_basic_logger_multithread: OK")
        .expect("Failed to log debug");
    debug!("#DBG: Releasing TEST_LOCK for test_basic_logger_multithread");
}

#[tokio::test]
async fn test_basic_logger_rotation() {
    debug!("#DBG: Acquiring TEST_LOCK for test_basic_logger_rotation");
    let _guard = timeout(TokioDuration::from_secs(60), TEST_LOCK.lock())
        .await
        .expect("Timeout waiting for TEST_LOCK");
    debug!("#DBG: Acquired TEST_LOCK for test_basic_logger_rotation");

    init_tracing().await;

    let log_dir = "test-logs";
    fs::create_dir_all(log_dir).expect("Failed to create test log directory");
    logger::debug(&format!("#DBG: Created log directory {}", log_dir))
        .expect("Failed to log debug");

    let locked_log = logger::get();
    let thread_id: u32;
    const LINES_LIMIT: u32 = 500;
    {
        let mut log = locked_log.lock().unwrap();
        thread_id = log.thread_id;
        log.set_limit_lines(LINES_LIMIT);
        log.debug(&format!("#DBG: Set lines limit to {} for thread_id={}", LINES_LIMIT, thread_id))
            .expect("Failed to log debug");
    }

    for i in 0..600 {
        logger::info(&format!("~C92Test message {}~C00", i))
            .expect("Failed to log");
    }

    sleep(TokioDuration::from_millis(1000)).await;
    logger::debug("#DBG: Finished waiting for file writes in test_basic_logger_rotation")
        .expect("Failed to log debug");

    let log_file = format!("{}/{}{}.log", log_dir, get_log_prefix(), thread_id);
    logger::debug(&format!("#DBG: Checking log file {}", log_file))
        .expect("Failed to log debug");
    assert!(Path::new(&log_file).exists(), "Log file {} not found", log_file);
    let content = fs::read_to_string(&log_file).expect("Failed to read log file");
    let clean_content = strip_ansi(&content);
    logger::debug(&format!("#DBG: Log file {} contents:\n{}", log_file, clean_content))
        .expect("Failed to log debug");

    let rotated_file_pattern = format!("{}/{}{}_*.log", log_dir, get_log_prefix(), thread_id);
    logger::debug(&format!("#DBG: Checking rotated files with pattern {}", rotated_file_pattern))
        .expect("Failed to log debug");
    let rotated_files: Vec<_> = timeout(
        TokioDuration::from_secs(5),
        async {
            glob::glob(&rotated_file_pattern)
                .expect("Failed to read glob pattern")
                .filter_map(Result::ok)
                .collect()
        },
    )
    .await
    .expect("Glob operation timed out");
    assert!(
        !rotated_files.is_empty(),
        "No rotated log files found for thread_id {}",
        thread_id
    );
    for rotated_file in rotated_files {
        assert!(rotated_file.exists(), "Rotated file {} not found", rotated_file.display());
        let content = fs::read_to_string(&rotated_file).expect("Failed to read rotated file");
        let clean_content = strip_ansi(&content);
        logger::debug(&format!("#DBG: Rotated file {} contents:\n{}", rotated_file.display(), clean_content))
            .expect("Failed to log debug");
        assert!(
            clean_content.contains(" [INFO] Test message "),
            "Expected ' [INFO] Test message ' in {}, got:\n{}",
            rotated_file.display(),
            clean_content
        );
    }

    list_log_dir(log_dir).await;
    logger::debug("#DBG: test_basic_logger_rotation: OK")
        .expect("Failed to log debug");
    debug!("#DBG: Releasing TEST_LOCK for test_basic_logger_rotation");
}