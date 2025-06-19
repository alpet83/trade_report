// Modified: 2025-06-19 12:34:00 EEST
// xaiArtifact: artifact_id="26837d4d-3726-4c1e-ac04-b900197934b7", artifact_version_id="1c2d3e4f-5a6b-7c8d-9e0f-1a2b3c4d5e6f"

use chrono::{DateTime, Utc, Duration, Timelike};
use tracing::{info};
use tracing_subscriber::EnvFilter;
use async_trait::async_trait;

use crate::{
    entities::{trade::{Trade, Order, TradeSignal}, account_data::{DepositHistoryRow, FundsHistoryRow}, ticker::TickerInfo, position::PositionHistory, report::ReportConfig, account::TradingAccount, exchange::Exchange},
    services::deposit_basic_report::{DepositBasicReport, generate_deposit_report},
    services::{chart::ChartReportGenerator, equity_report::EquityReportGenerator},
    db::{mysql::TradeDataSource, load_equity_data::LoadEquityData},
};
use std::sync::Arc;

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .try_init();
}

struct MockTradeDataSource {
    funds_history: Vec<FundsHistoryRow>,
    deposit_history: Vec<DepositHistoryRow>,
    account_id: i32,
}

#[async_trait]
impl TradeDataSource for MockTradeDataSource {
    async fn get_funds_history(
        &self,
        _exchange: &str,
        account_id: i32,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<FundsHistoryRow>, String> {
        if account_id != self.account_id {
            Ok(vec![])
        } else {
            Ok(self.funds_history.clone())
        }
    }

    async fn get_deposit_history(
        &self,
        _exchange: &str,
        account_id: i32,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<DepositHistoryRow>, String> {
        if account_id != self.account_id {
            Ok(vec![])
        } else {
            Ok(self.deposit_history.clone())
        }
    }

    async fn get_trades(
        &self,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
        _exchange: &str,
        _pair_id: Option<i32>,
        account_id: i32,
    ) -> Result<Vec<Trade>, String> {
        if account_id != self.account_id {
            Ok(vec![])
        } else {
            Err("Not implemented".to_string())
        }
    }

    async fn get_orders(
        &self,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
        _exchange: &str,
        _pair_id: Option<i32>,
        account_id: i32,
        _status: Option<&str>,
    ) -> Result<Vec<Order>, String> {
        if account_id != self.account_id {
            Ok(vec![])
        } else {
            Err("Not implemented".to_string())
        }
    }

    async fn get_candle_price(
        &self,
        _ts: DateTime<Utc>,
        _exchange: &str,
        _pair: &str,
        _use_clickhouse: bool,
    ) -> Result<f64, String> {
        Err("Not implemented".to_string())
    }

    async fn get_ticker_info(
        &self,
        _exchange: &str,
        _pair_id: i32,
    ) -> Result<TickerInfo, String> {
        Err("Not implemented".to_string())
    }

    async fn get_position_history(
        &self,
        _exchange: &str,
        _pair_id: i32,
        _account_id: i32,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<PositionHistory>, String> {
        Err("Not implemented".to_string())
    }

    async fn get_trade_signals(
        &self,
        _exchange: &str,
        _account_id: i32,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<TradeSignal>, String> {
        Err("Not implemented".to_string())
    }

    async fn get_report_configs(
        &self,
        _exchange: &str,
    ) -> Result<Vec<ReportConfig>, String> {
        Err("Not implemented".to_string())
    }
}

#[async_trait]
impl LoadEquityData for MockTradeDataSource {
    async fn load_equity_data(
        &self,
        _exchange: &str,
        account_id: i32,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
        value_column: &str,
    ) -> Result<Vec<(DateTime<Utc>, f32)>, String> {
        if account_id != self.account_id {
            Ok(vec![])
        } else {
            let mut equity_points = Vec::new();
            let mut deposit_sum = 0.0;

            for fund in &self.funds_history {
                deposit_sum += self.deposit_history.iter()
                    .filter(|d| d.ts <= fund.ts)
                    .map(|d| d.amount)
                    .sum::<f32>();

                let value = match value_column {
                    "value_btc" => fund.value_btc,
                    _ => fund.value,
                };
                let equity = value + deposit_sum;

                let ts = fund.ts
                    .with_second(0)
                    .expect("Invalid datetime")
                    .with_nanosecond(0)
                    .expect("Invalid datetime");

                equity_points.push((ts, equity));
            }

            Ok(equity_points)
        }
    }
}

#[tokio::test]
async fn test_generate_deposit_report_value() {
    init_tracing();

    let end_ts = Utc::now();
    let start_ts = end_ts - Duration::hours(24);
    let history = vec![
        FundsHistoryRow {
            ts: start_ts,
            value: 99979.9,
            value_btc: 0.951331,
            position_coef: 0.013,
        },
        FundsHistoryRow {
            ts: end_ts,
            value: 100027.0,
            value_btc: 0.950969,
            position_coef: 0.013,
        },
    ];

    let db = MockTradeDataSource { 
        funds_history: history, 
        deposit_history: vec![],
        account_id: 379832 
    };
    let account = TradingAccount::new(
        "379832".to_string(),
        "bitmex2_bot".to_string(),
        Arc::new(Exchange::new("bitmex".to_string())),
        true,
    );

    let report = generate_deposit_report(&db, &account, start_ts, end_ts, Some("value"))
        .await
        .expect("Failed to generate report");

    assert_eq!(report.account_id, "379832");
    assert_eq!(report.exchange, "bitmex");
    assert_eq!(report.start_value, 99979.9);
    assert_eq!(report.end_value, 100027.0);
    assert_eq!(report.value_column, "value");
    assert!((report.change_percent - 0.0471).abs() < 0.001, "Expected change_percent ≈ 0.0471, got {}", report.change_percent);

    info!("Successfully tested deposit basic report with value column");
}

#[tokio::test]
async fn test_generate_deposit_report_value_btc() {
    init_tracing();

    let end_ts = Utc::now();
    let start_ts = end_ts - Duration::hours(24);
    let history = vec![
        FundsHistoryRow {
            ts: start_ts,
            value: 99979.9,
            value_btc: 0.951331,
            position_coef: 0.013,
        },
        FundsHistoryRow {
            ts: end_ts,
            value: 100027.0,
            value_btc: 0.950969,
            position_coef: 0.013,
        },
    ];

    let db = MockTradeDataSource { 
        funds_history: history, 
        deposit_history: vec![],
        account_id: 379832 
    };
    let account = TradingAccount::new(
        "379832".to_string(),
        "bitmex2_bot".to_string(),
        Arc::new(Exchange::new("bitmex".to_string())),
        true,
    );

    let report = generate_deposit_report(&db, &account, start_ts, end_ts, Some("value_btc"))
        .await
        .expect("Failed to generate report");

    assert_eq!(report.account_id, "379832");
    assert_eq!(report.exchange, "bitmex");
    assert_eq!(report.start_value, 0.951331);
    assert_eq!(report.end_value, 0.950969);
    assert_eq!(report.value_column, "value_btc");
    assert!((report.change_percent - (-0.0381)).abs() < 0.001, "Expected change_percent ≈ -0.0381, got {}", report.change_percent);

    info!("Successfully tested deposit basic report with value_btc column");
}

#[tokio::test]
async fn test_generate_deposit_report_default() {
    init_tracing();

    let end_ts = Utc::now();
    let start_ts = end_ts - Duration::hours(24);
    let history = vec![
        FundsHistoryRow {
            ts: start_ts,
            value: 99979.9,
            value_btc: 0.951331,
            position_coef: 0.013,
        },
        FundsHistoryRow {
            ts: end_ts,
            value: 100027.0,
            value_btc: 0.950969,
            position_coef: 0.013,
        },
    ];

    let db = MockTradeDataSource { 
        funds_history: history, 
        deposit_history: vec![],
        account_id: 379832 
    };
    let account = TradingAccount::new(
        "379832".to_string(),
        "bitmex2_bot".to_string(),
        Arc::new(Exchange::new("bitmex".to_string())),
        true,
    );

    let report = generate_deposit_report(&db, &account, start_ts, end_ts, None)
        .await
        .expect("Failed to generate report");

    assert_eq!(report.account_id, "379832");
    assert_eq!(report.exchange, "bitmex");
    assert_eq!(report.start_value, 99979.9);
    assert_eq!(report.end_value, 100027.0);
    assert_eq!(report.value_column, "value");
    assert!((report.change_percent - 0.0471).abs() < 0.001, "Expected change_percent ≈ 0.0471, got {}", report.change_percent);

    info!("Successfully tested deposit basic report with default column");
}

#[tokio::test]
async fn test_generate_deposit_report_wrong_account_id() {
    init_tracing();

    let end_ts = Utc::now();
    let start_ts = end_ts - Duration::hours(24);
    let history = vec![
        FundsHistoryRow {
            ts: start_ts,
            value: 99979.9,
            value_btc: 0.951331,
            position_coef: 0.013,
        },
        FundsHistoryRow {
            ts: end_ts,
            value: 100027.0,
            value_btc: 0.950969,
            position_coef: 0.013,
        },
    ];

    let db = MockTradeDataSource { 
        funds_history: history, 
        deposit_history: vec![],
        account_id: 379832 
    };
    let account = TradingAccount::new(
        "864208".to_string(),
        "bitmex2_bot".to_string(),
        Arc::new(Exchange::new("bitmex".to_string())),
        true,
    );

    let result = generate_deposit_report(&db, &account, start_ts, end_ts, Some("value_btc"))
        .await;

    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "No funds history found");

    info!("Successfully tested deposit basic report with wrong account_id");
}

#[tokio::test]
async fn test_generate_deposit_report_specific_time() {
    init_tracing();

    let end_ts = Utc::now();
    let start_ts = end_ts - Duration::hours(12);
    let history = vec![
        FundsHistoryRow {
            ts: start_ts,
            value: 99979.9,
            value_btc: 0.951331,
            position_coef: 0.013,
        },
        FundsHistoryRow {
            ts: end_ts,
            value: 100027.0,
            value_btc: 0.950969,
            position_coef: 0.013,
        },
    ];

    let db = MockTradeDataSource { 
        funds_history: history, 
        deposit_history: vec![],
        account_id: 379832 
    };
    let account = TradingAccount::new(
        "379832".to_string(),
        "bitmex2_bot".to_string(),
        Arc::new(Exchange::new("bitmex".to_string())),
        true,
    );

    let report = generate_deposit_report(&db, &account, start_ts, end_ts, Some("value_btc"))
        .await
        .expect("Failed to generate report");

    assert_eq!(report.account_id, "379832");
    assert_eq!(report.exchange, "bitmex");
    assert_eq!(report.start_value, 0.951331);
    assert_eq!(report.end_value, 0.950969);
    assert_eq!(report.value_column, "value_btc");
    assert!((report.change_percent - (-0.0381)).abs() < 0.001, "Expected change_percent ≈ -0.0381, got {}", report.change_percent);

    info!("Successfully tested deposit basic report with specific start_ts and end_ts");
}

#[tokio::test]
async fn test_load_equity_data() {
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
            amount: 500.0,
        },
    ];

    let db = MockTradeDataSource {
        funds_history,
        deposit_history,
        account_id: 379832,
    };
    let account = TradingAccount::new(
        "379832".to_string(),
        "bitmex2_bot".to_string(),
        Arc::new(Exchange::new("bitmex".to_string())),
        true,
    );

    let equity_points = db.load_equity_data(
        "bitmex",
        379832,
        start_ts,
        end_ts,
        "value_btc",
    )
    .await
    .expect("Failed to load equity data");

    assert_eq!(equity_points.len(), 2);
    assert_eq!(equity_points[0].1, 0.48); // -0.02 + 0.5
    assert_eq!(equity_points[1].1, 0.475); // -0.025 + 0.5
    assert_eq!(equity_points[0].0.second(), 0);
    assert_eq!(equity_points[1].0.second(), 0);

    info!("Successfully tested load_equity_data");
}

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
            amount: 500.0,
        },
    ];

    let db = MockTradeDataSource {
        funds_history,
        deposit_history,
        account_id: 379832,
    };
    let account = TradingAccount::new(
        "379832".to_string(),
        "bitmex2_bot".to_string(),
        Arc::new(Exchange::new("bitmex".to_string())),
        true,
    );

    let generator = EquityReportGenerator::new(&db, 800, 600);
    let svg_data = generator.generate_svg(&account, start_ts, end_ts, Some("value_btc"))
        .await
        .expect("Failed to generate SVG chart");

    assert!(!svg_data.is_empty());
    let svg_str = String::from_utf8(svg_data).expect("Invalid SVG data");
    assert!(svg_str.contains("stroke=\"rgb(0,128,0)\"")); // Green line (positive equity)

    info!("Successfully tested equity chart generation");
}