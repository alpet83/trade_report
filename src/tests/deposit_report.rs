// /src/tests/deposit_report.rs
// Modified: 2025-06-22 10:25:00 EEST

// ... (previous imports and code unchanged until test_generate_equity_chart)

// Tests equity chart generation with light and dark themes
#[tokio::test]
async fn test_generate_equity_chart() {
    init_tracing();

    let end_ts = Utc::now();
    let start_ts = end_ts - Duration::hours(12);
    let funds_history = vec![
        FundsHistoryRow {
            ts: start_ts,
            value: -1000.0,
            value_btc: -0.02,
            position_coef: 0.013,
        },
        FundsHistoryRow {
            ts: end_ts,
            value: -1200.0,
            value_btc: -0.025,
            position_coef: 0.013,
        },
    ];
    let deposit_history = vec![
        DepositHistoryRow {
            ts: start_ts,
            withdrawal: false,
            value_usd: 500.0,
            value_btc: 0.0,
        },
    ];

    let db = MockTradeDataSource {
        funds_history,
        deposit_history,
        account_id: 379832,
    };
    let account = TradingAccount::new(
        379832,
        "bitmex2_bot".to_string(),
        Arc::new(Exchange::new("bitmex".to_string())),
        true,
    );

    let generator = EquityReportGenerator::new(&db, 800, 600);

    // Test light theme
    let svg_data = generator.generate_svg(&account, start_ts, end_ts, Some("value_btc"), false)
        .await
        .expect("Failed to generate SVG chart");

    assert!(!svg_data.is_empty());
    let svg_str = String::from_utf8(svg_data).expect("Invalid SVG data");
    assert!(svg_str.contains("stroke=\"rgb(0,128,0)\"")); // Green line
    assert!(svg_str.contains("fill=\"rgb(0,128,0)\"")); // Green fill
    assert!(svg_str.contains("fill=\"rgb(255,255,255)\"")); // White background

    // Test dark theme
    let svg_data_dark = generator.generate_svg(&account, start_ts, end_ts, Some("value_btc"), true)
        .await
        .expect("Failed to generate SVG chart");

    assert!(!svg_data_dark.is_empty());
    let svg_str_dark = String::from_utf8(svg_data_dark).expect("Invalid SVG data");
    assert!(svg_str_dark.contains("stroke=\"rgb(0,128,0)\"")); // Green line
    assert!(svg_str_dark.contains("fill=\"rgb(0,128,0)\"")); // Green fill
    assert!(svg_str_dark.contains("fill=\"rgb(30,30,30)\"")); // Dark gray background
    assert!(svg_str_dark.contains("fill=\"rgb(255,255,255)\"")); // White font

    info!("Successfully tested equity chart generation with light and dark themes");
}

// ... (other tests unchanged)