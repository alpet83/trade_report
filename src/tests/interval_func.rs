use chrono::{DateTime, Utc};
use serde_json::Value;
use std::fs;
use tracing::{debug};
use tracing_subscriber::EnvFilter;

use crate::common::interval_func::{
    adjust_to_monday,
    adjust_to_first_of_month,
    adjust_to_first_of_quarter,
    adjust_to_first_of_year,
    MONTH_SECONDS,
};

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("debug".parse().unwrap()))
        .try_init();
}

#[tokio::test]
async fn test_interval_functions() {
    init_tracing();

    let expected_results: Value = serde_json::from_str(
        &fs::read_to_string("interval_expected.json").expect("Failed to read interval_expected.json")
    ).expect("Failed to parse interval_expected.json");

    // Test adjust_to_monday
    let monday_tests = expected_results["monday"]
        .as_array()
        .expect("Expected monday array");
    for test in monday_tests {
        let input = DateTime::parse_from_rfc3339(test["input"].as_str().unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let expected = DateTime::parse_from_rfc3339(test["expected"].as_str().unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let result = adjust_to_monday(input);
        debug!("#DBG: adjust_to_monday: input={}, expected={}, got={}", input, expected, result);
        assert_eq!(
            result, expected,
            "adjust_to_monday failed for input {}: expected {}, got {}",
            input, expected, result
        );
    }

    // Test adjust_to_first_of_month
    let month_tests = expected_results["month"]
        .as_array()
        .expect("Expected month array");
    for test in month_tests {
        let input = DateTime::parse_from_rfc3339(test["input"].as_str().unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let interval_seconds = test["interval_seconds"].as_i64().unwrap();
        let week_align = test["week_align"].as_bool().unwrap_or(false);
        let expected = DateTime::parse_from_rfc3339(test["expected"].as_str().unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let result = adjust_to_first_of_month(input, interval_seconds, week_align);
        debug!("#DBG: adjust_to_first_of_month: input={}, interval_seconds={}, week_align={}, expected={}, got={}", 
            input, interval_seconds, week_align, expected, result);
        assert_eq!(
            result, expected,
            "adjust_to_first_of_month failed for input {}: expected {}, got {}",
            input, expected, result
        );
    }

    // Test adjust_to_first_of_quarter
    let quarter_tests = expected_results["quarter"]
        .as_array()
        .expect("Expected quarter array");
    for test in quarter_tests {
        let input = DateTime::parse_from_rfc3339(test["input"].as_str().unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let week_align = test["week_align"].as_bool().unwrap_or(false);
        let expected = DateTime::parse_from_rfc3339(test["expected"].as_str().unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let result = adjust_to_first_of_quarter(input, week_align);
        debug!("#DBG: adjust_to_first_of_quarter: input={}, week_align={}, expected={}, got={}", 
            input, week_align, expected, result);
        assert_eq!(
            result, expected,
            "adjust_to_first_of_quarter failed for input {}: expected {}, got {}",
            input, expected, result
        );
    }

    // Test adjust_to_first_of_year
    let year_tests = expected_results["year"]
        .as_array()
        .expect("Expected year array");
    for test in year_tests {
        let input = DateTime::parse_from_rfc3339(test["input"].as_str().unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let week_align = test["week_align"].as_bool().unwrap_or(false);
        let expected = DateTime::parse_from_rfc3339(test["expected"].as_str().unwrap())
            .unwrap()
            .with_timezone(&Utc);
        let result = adjust_to_first_of_year(input, week_align);
        debug!("#DBG: adjust_to_first_of_year: input={}, week_align={}, expected={}, got={}", 
            input, week_align, expected, result);
        assert_eq!(
            result, expected,
            "adjust_to_first_of_year failed for input {}: expected {}, got {}",
            input, expected, result
        );
    }

    // Additional tests for adjust_to_monday edge cases
    let additional_monday_tests = vec![
        (
            "2024-12-31T23:59:59.999Z",
            "2024-12-30T00:00:00Z", // Monday of the week containing Dec 31, 2024
        ),
        (
            "2025-01-01T12:30:45.123Z",
            "2024-12-30T00:00:00Z", // Monday of the week containing Jan 1, 2025
        ),
        (
            "2025-03-01T12:00:00Z",
            "2025-02-24T00:00:00Z", // Monday of the week containing Mar 1, 2025
        ),
        (
            "2024-01-01T12:00:00Z",
            "2024-01-01T00:00:00Z", // Monday of the week containing Jan 1, 2024
        ),
    ];

    for (input_str, expected_str) in additional_monday_tests {
        let input = DateTime::parse_from_rfc3339(input_str)
            .unwrap()
            .with_timezone(&Utc);
        let expected = DateTime::parse_from_rfc3339(expected_str)
            .unwrap()
            .with_timezone(&Utc);
        let result = adjust_to_monday(input);
        debug!("#DBG: Additional adjust_to_monday: input={}, expected={}, got={}", 
            input, expected, result);
        assert_eq!(
            result, expected,
            "Additional adjust_to_monday failed: input {}, expected {}, got {}",
            input, expected, result
        );
    }
}