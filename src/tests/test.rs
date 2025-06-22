// /src/tests/rtm.rs
// Modified: 2025-06-22 10:35:00 EEST

use serde_qs;
use tracing::{info};
use tracing_subscriber::EnvFilter;

use crate::api::rtm::DepositReportQuery;

// Initializes tracing for test output
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .try_init();
}

// Tests deserialization of DepositReportQuery with dark parameter
#[test]
fn test_deposit_report_query_deserialization() {
    init_tracing();

    // Test dark=1
    let query_str = "dark=1";
    let query: DepositReportQuery = serde_qs::from_str(query_str).expect("Failed to deserialize dark=1");
    assert_eq!(query.dark, Some(true), "Expected dark=1 to deserialize as true");

    // Test dark=0
    let query_str = "dark=0";
    let query: DepositReportQuery = serde_qs::from_str(query_str).expect("Failed to deserialize dark=0");
    assert_eq!(query.dark, Some(false), "Expected dark=0 to deserialize as false");

    // Test dark=true
    let query_str = "dark=true";
    let query: DepositReportQuery = serde_qs::from_str(query_str).expect("Failed to deserialize dark=true");
    assert_eq!(query.dark, Some(true), "Expected dark=true to deserialize as true");

    // Test dark=false
    let query_str = "dark=false";
    let query: DepositReportQuery = serde_qs::from_str(query_str).expect("Failed to deserialize dark=false");
    assert_eq!(query.dark, Some(false), "Expected dark=false to deserialize as false");

    // Test invalid dark value
    let query_str = "dark=invalid";
    let result = serde_qs::from_str::<DepositReportQuery>(query_str);
    assert!(result.is_err(), "Expected error for invalid dark value");

    // Test no dark parameter
    let query_str = "";
    let query: DepositReportQuery = serde_qs::from_str(query_str).expect("Failed to deserialize empty query");
    assert_eq!(query.dark, None, "Expected no dark parameter to deserialize as None");

    info!("Successfully tested DepositReportQuery deserialization with dark parameter");
}