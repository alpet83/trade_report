// /src/tests/deposit_report.rs
// Modified: 2025-06-22 14:30:00 EEST

use chrono::{DateTime, Utc, Duration, Timelike};
use tracing::{info};
use tracing_subscriber::EnvFilter;
use async_trait::async_trait;
use std::sync::Arc;

use crate::{
    entities::{
        trade::{Trade, Order, TradeSignal},
        account_data::{DepositHistoryRow, FundsHistoryRow},
        ticker::TickerInfo,
        position::PositionHistory,
        report::ReportConfig,
        account::TradingAccount,
        exchange::Exchange,
    },
    services::{
        deposit_basic_report::{DepositBasicReport, generate_deposit_report},
        chart::ChartReportGenerator,
        equity_report::EquityReportGenerator,
    },
    db::{
        mysql::MySqlDataSource,
        trade_data::TradeDataSource,
        load_equity_data::LoadEquityData,
    },
};

// Initializes tracing for test output
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .try_init();
}

// Mock implementation of TradeDataSource for testing
struct MockTradeDataSource {
    funds_history: Vec<FundsHistoryRow>,
    deposit_history: Vec<DepositHistoryRow>,
    account_id: i32,
}

#[async_trait]
impl TradeDataSource for MockTradeDataSource {
    // Fetches mock funds history for testing
    async fn get_funds_history(
        &self,
        account: &TradingAccount,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<FundsHistoryRow>, String> {
        if account.account_id as i32 != self.account_id {
            Ok(vec![])
        } else {
            Ok(self.funds_history.clone())
        }
    }

    // Fetches mock aggregated funds history for testing
    async fn get_funds_history_aggregated(
        &self,
        account: &TradingAccount,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<FundsHistoryRow>, String> {
        if account.account_id as i32 != self.account_id {
            Ok(vec![])
        } else {
            Ok(self.funds_history.clone())
        }
    }

    // Fetches mock deposit history for testing
    async fn get_deposit_history(
        &self,
        account: &TradingAccount,
        _end: DateTime<Utc>,
    ) -> Result<Vec<DepositHistoryRow>, String> {
        if account.account_id as i32 != self.account_id {
            Ok(vec![])
        } else {
            Ok(self.deposit_history.clone())
        }
    }

    async fn get_trades(
        &self,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
        _account: &TradingAccount,
        _pair_id: Option<u32>,
    ) -> Result<Vec<Trade>, String> {
        Ok(vec![])
    }

    async fn get_orders(
        &self,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
        _account: &TradingAccount,
        _pair_id: Option<u32>,
        _status: Option<&str>,
    ) -> Result<Vec<Order>, String> {
        Ok(vec![])
    }

    async fn get_position_history(
        &self,
        _account: &TradingAccount,
        _pair_id: u32,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<PositionHistory>, String> {
        Ok(vec![])
    }

    async fn get_trade_signals(
        &self,
        _account: &TradingAccount,
        _start: DateTime<Utc>,
        _end: DateTime<Utc>,
    ) -> Result<Vec<TradeSignal>, String> {
        Ok(vec![])
    }

    async fn get_report_configs(
        &self,
        _exchange: &str,
    ) -> Result<Vec<ReportConfig>, String> {
        Ok(vec![])
    }
}

#[async_trait]
impl LoadEquityData for MockTradeDataSource {
    async fn load_equity_data(
        &self,
        account: &TradingAccount,
        start_ts: DateTime<Utc>,
        end_ts: DateTime<Utc>,
        _value_column: &str,
    ) -> Result<Vec<(DateTime<Utc>, f32)>, String> {
        if account.account_id as i32 != self.account_id {
            Ok(vec![])
        } else {
            let funds = self.get_funds_history(account, start_ts, end_ts)
                .await?;
            let deposits = self.get_deposit_history(account, end_ts)
                .await?;
            let mut equity_points = Vec::new();
            let mut accum_usd = 0.0;
            let mut accum_btc = 0.0;
            let mut fund_idx = 0;

            let mut sorted_deposits = deposits.clone();
            sorted_deposits.sort_by(|a, b| a.ts.cmp(&b.ts));
            sorted_deposits.push(DepositHistoryRow {
                ts: end_ts + chrono::Duration::seconds(1),
                withdrawal: false,
                value_usd: 0.0,
                value_btc: 0.0,
            });

            for dep in sorted_deposits {
                let dep_ts = dep.ts;
                while fund_idx < funds.len() && funds[fund_idx].ts <= dep_ts {
                    let fund = &funds[fund_idx];
                    let ts = fund.ts
                        .with_second(0)
                        .expect("Invalid datetime")
                        .with_nanosecond(0)
                        .expect("Invalid datetime");
                    equity_points.push((ts, fund.value_btc - accum_btc));
                    fund_idx += 1;
                }
                let sign = if dep.withdrawal { -1.0 } else { 1.0 };
                accum_usd += dep.value_usd * sign;
                accum_btc += dep.value_btc * sign;
            }
            Ok(equity_points)
        }
    }
}

// Tests equity chart generation with light, dark, and weekly themes
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
            ts: start_ts + Duration::hours(2),
            value: -1100.0,
            value_btc: -0.022,
            position_coef: 0.013,
        },
        FundsHistoryRow {
            ts: start_ts + Duration::hours(4),
            value: -1200.0,
            value_btc: -0.025,
            position_coef: 0.013,
        },
        FundsHistoryRow {
            ts: start_ts + Duration::hours(6),
            value: -1300.0,
            value_btc: -0.028,
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

    let db = Arc::new(MockTradeDataSource {
        funds_history,
        deposit_history,
        account_id: 379832,
    });
    MySqlDataSource::init_db_conn_with_mock(db.clone()).await;

    let account = TradingAccount::new(
        379832,
        "bitmex2_bot".to_string(),
        Arc::new(Exchange::new("bitmex".to_string()).await),
        true,
    );

    let generator = EquityReportGenerator::new(800, 600);

    // Test light theme (period < 2 days, expect Y-m-d H format)
    let svg_data = generator
        .generate_svg(&account, start_ts, end_ts, Some("value_btc"), false, None)
        .await
        .expect("Failed to generate SVG chart");

    assert!(!svg_data.is_empty());
    let svg_str = String::from_utf8(svg_data).expect("Invalid SVG data");
    assert!(svg_str.contains("stroke=\"rgb(0,128,0)\"")); // Green line
    assert!(svg_str.contains("fill=\"rgb(0,128,0)\"")); // Green fill
    assert!(svg_str.contains("fill=\"rgb(255,255,255)\"")); // White background
    assert!(
        svg_str.contains(&start_ts.format("%Y-%m-%d %H").to_string()),
        "Expected time format Y-m-d H"
    );

    // Test dark theme (period < 2 days, expect Y-m-d H format)
    let svg_data_dark = generator
        .generate_svg(&account, start_ts, end_ts, Some("value_btc"), true, None)
        .await
        .expect("Failed to generate SVG chart");

    assert!(!svg_data_dark.is_empty());
    let svg_str_dark = String::from_utf8(svg_data_dark).expect("Invalid SVG data");
    assert!(svg_str_dark.contains("stroke=\"rgb(0,128,0)\"")); // Green line
    assert!(svg_str_dark.contains("fill=\"rgb(0,128,0)\"")); // Green fill
    assert!(svg_str_dark.contains("fill=\"rgb(30,30,30)\"")); // Dark gray background
    assert!(svg_str_dark.contains("fill=\"rgb(255,255,255)\"")); // White font
    assert!(
        svg_str_dark.contains(&start_ts.format("%Y-%m-%d %H").to_string()),
        "Expected time format Y-m-d H"
    );

    // Test weekly period (1 year, expect Y-m format)
    let yearly_start_ts = end_ts - Duration::days(365);
    let svg_data_weekly = generator
        .generate_svg(&account, yearly_start_ts, end_ts, Some("value_btc"), false, Some("weekly"))
        .await
        .expect("Failed to generate SVG chart");

    assert!(!svg_data_weekly.is_empty());
    let svg_str_weekly = String::from_utf8(svg_data_weekly).expect("Invalid SVG data");
    assert!(svg_str_weekly.contains("stroke=\"rgb(0,128,0)\"")); // Green line
    assert!(svg_str_weekly.contains("fill=\"rgb(0,128,0)\"")); // Green fill
    assert!(svg_str_weekly.contains("fill=\"rgb(255,255,255)\"")); // White background
    assert!(
        svg_str_weekly.contains(&end_ts.format("%Y-%m").to_string()),
        "Expected time format Y-m"
    );

    // Test 10-day period (expect Y-m-d format)
    let ten_day_start_ts = end_ts - Duration::days(10);
    let svg_data_ten_day = generator
        .generate_svg(&account, ten_day_start_ts, end_ts, Some("value_btc"), false, None)
        .await
        .expect("Failed to generate SVG chart");

    assert!(!svg_data_ten_day.is_empty());
    let svg_str_ten_day = String::from_utf8(svg_data_ten_day).expect("Invalid SVG data");
    assert!(svg_str_ten_day.contains(&end_ts.format("%Y-%m-%d").to_string()), "Expected time format Y-m-d");

    info!("Successfully tested equity chart generation with light, dark, weekly, and 10-day themes");
}