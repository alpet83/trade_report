// /src/api/rtm.rs
// Modified: 2025-06-25 09:21 EEST

use axum::{Router, routing::get, Json, extract::Query, http::{Response, HeaderMap, StatusCode}};
use axum::response::IntoResponse;
use serde_json::{Map, Value};
use tokio::time::{timeout, Duration};
use tracing::{info, error, debug};
use backtrace::Backtrace;
use serde::{Deserialize, Deserializer};
use chrono::{DateTime, Utc, Duration as ChronoDuration};

use crate::{
    entities::account::{TradingAccount, get_account_manager, resolve_account},
    entities::cache::TradesCache,
    entities::trades_aggregator::{TradesAggregator, CalcMethod},
    entities::trade::Trade,
    entities::task::TaskStatus,
    services::deposit_basic_report::{DepositBasicReport, generate_deposit_report},
    services::{chart::ChartReportGenerator, equity_report::EquityReportGenerator},
    db::mysql::MySqlDataSource,
    common::time::resolve_time_range,
    common::consts::BTC_PAIR_ID,
    config::Config,
    logs::app_error::AppError,
};

// Deserializes dark parameter from "1", "0", "true", or "false"
fn deserialize_dark<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s.as_deref() {
        Some("1") | Some("true") => Ok(Some(true)),
        Some("0") | Some("false") => Ok(Some(false)),
        Some(other) => Err(serde::de::Error::custom(format!("Invalid dark value: {}", other))),
        None => Ok(None),
    }
}

// Defines query parameters for deposit report, equity chart, and trades aggregation requests
#[derive(Deserialize)]
pub struct DepositReportQuery {
    exchange: Option<String>,
    account_id: Option<u32>,
    applicant: Option<String>,
    period: Option<i64>,
    period_type: Option<String>, // Supports weekly period
    value_column: Option<String>,
    start_ts: Option<String>,
    end_ts: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    #[serde(deserialize_with = "deserialize_dark")]
    pub dark: Option<bool>, // Supports 1, 0, true, false
    coarse_interval: Option<String>, // e.g., "1d", "7d", "30d", "90d", "365d"
    precise_comb: Option<String>, // "1" for precise aggregation
    week_align: Option<String>, // "1" or "true" for week-aligned aggregation
}

// Configures API routes for account, report, and trades aggregation endpoints
pub fn routes() -> Router<()> {
    Router::new()
        .route("/accounts", get(get_accounts))
        .route("/deposit_report", get(get_deposit_report))
        .route("/equity_chart", get(get_equity_chart))
        .route("/trades_aggregated", get(get_trades_aggregated))
}

// Fetches list of trading accounts
async fn get_accounts() -> Result<Json<Map<String, Value>>, AppError> {
    info!("Starting request to fetch trading accounts");

    let result = timeout(Duration::from_secs(60), async {
        debug!("Acquiring read lock on TradingAccountManager");
        let manager = get_account_manager();
        let guard = manager.read().await;

        debug!("Read lock acquired, listing accounts");
        let accounts = guard.list_accounts();
        if accounts.is_empty() {
            error!("No trading accounts found");
            return Err(AppError::Internal("No trading accounts configured".to_string()));
        }

        let accounts: Vec<TradingAccount> = accounts.into_iter().cloned().collect();
        drop(guard);
        debug!("Read lock released, serializing accounts");

        let mut result = Map::new();
        for account in accounts {
            debug!("Serializing account: {}", account.account_id);
            let account_value = serde_json::to_value(&account)
                .map_err(|e| AppError::Internal(format!("Failed to serialize account {}: {}", account.account_id, e)))?;
            result.insert(account.applicant.clone(), account_value);
        }

        debug!("Serialization complete, returning {} accounts", result.len());
        Ok(Json(result))
    })
    .await;

    match result {
        Ok(Ok(json)) => {
            info!("Request completed, returning {} accounts", json.0.len());
            Ok(json)
        }
        Ok(Err(e)) => {
            error!("Request failed: {:?}", e);
            Err(e)
        }
        Err(_) => {
            let backtrace = Backtrace::new();
            error!("Request timed out after 60 seconds\nBacktrace:\n{:?}", backtrace);
            Err(AppError::Internal(format!("Request timed out after 60 seconds\nBacktrace:\n{:?}", backtrace)))
        }
    }
}

// Generates a deposit report based on query parameters
#[axum::debug_handler]
async fn get_deposit_report(Query(params): Query<DepositReportQuery>) -> Result<Json<DepositBasicReport>, AppError> {
    info!("Starting deposit report request");

    debug!("Validating query parameters");
    let account = resolve_account(
        params.exchange,
        params.account_id.map(|id| id.to_string()),
        params.applicant,
    ).await?;
    debug!("Selected account_id: {}, exchange: {}", account.account_id, account.exchange.name);

    let (start_ts, end_ts) = resolve_time_range(params.start_ts, params.end_ts, params.period, params.period_type.clone()).await?;

    debug!("Using time range: start_ts={}, end_ts={}", start_ts, end_ts);

    let result = timeout(Duration::from_secs(60), async {
        debug!("Using MySqlDataSource singleton");
        let report = generate_deposit_report(&account, start_ts, end_ts, params.value_column.as_deref())
            .await
            .map_err(|e| AppError::Internal(format!("Failed to generate report: {}", e)))?;

        debug!("Report generated successfully");
        Ok(Json(report))
    })
    .await;

    match result {
        Ok(Ok(json)) => {
            info!("Deposit report request completed");
            Ok(json)
        }
        Ok(Err(e)) => {
            error!("Deposit report request failed: {:?}", e);
            Err(e)
        }
        Err(_) => {
            let backtrace = Backtrace::new();
            error!("Request timed out after 60 seconds\nBacktrace:\n{:?}", backtrace);
            Err(AppError::Internal(format!("Request timed out after 60 seconds\nBacktrace:\n{:?}", backtrace)))
        }
    }
}

// Generates an equity chart based on query parameters
#[axum::debug_handler]
async fn get_equity_chart(Query(params): Query<DepositReportQuery>) -> Result<axum::http::Response<String>, AppError> {
    info!("Starting equity chart request");

    debug!("Validating query parameters");
    let account = resolve_account(
        params.exchange,
        params.account_id.map(|id| id.to_string()),
        params.applicant,
    ).await?;
    debug!("Selected account_id: {}, exchange: {}", account.account_id, account.exchange.name);

    let (start_ts, end_ts) = resolve_time_range(params.start_ts, params.end_ts, params.period, params.period_type.clone()).await?;

    let width = params.width.unwrap_or(800);
    let height = params.height.unwrap_or(600);
    let dark = params.dark.unwrap_or(false); // Default to light theme

    debug!("Using time range: start_ts={}, end_ts={}, width={}, height={}, dark={}, period_type={:?}", 
        start_ts, end_ts, width, height, dark, params.period_type);

    let result = timeout(Duration::from_secs(60), async {
        debug!("Generating equity chart for account_id={} on {}", account.account_id, account.exchange.name);
        let generator = EquityReportGenerator::new(width, height);
        let svg_data = generator.generate_svg(&account, start_ts, end_ts, params.value_column.as_deref(), dark, params.period_type.as_deref())
            .await
            .map_err(|e| AppError::Internal(format!("Failed to generate chart: {}", e)))?;

        debug!("Chart generated successfully");
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", "image/svg+xml".parse().unwrap());
        Ok(axum::http::Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "image/svg+xml")
            .body(String::from_utf8(svg_data).unwrap())
            .unwrap())
    })
    .await;

    match result {
        Ok(Ok(response)) => {
            info!("Equity chart request completed");
            Ok(response)
        }
        Ok(Err(e)) => {
            error!("Request failed: {:?}", e);
            Err(e)
        }
        Err(_) => {
            let backtrace = Backtrace::new();
            error!("Request timed out after 60 seconds\nBacktrace:\n{:?}", backtrace);
            Err(AppError::Internal(format!("Request timed out after 60 seconds\nBacktrace:\n{:?}", backtrace)))
        }
    }
}

// Generates aggregated trades based on query parameters
#[axum::debug_handler]
async fn get_trades_aggregated(Query(params): Query<DepositReportQuery>) -> Result<Json<Vec<Trade>>, AppError> {
    info!("Starting aggregated trades request");

    debug!("Validating query parameters");
    let account = resolve_account(
        params.exchange,
        params.account_id.map(|id| id.to_string()),
        params.applicant,
    ).await?;
    debug!("Selected account_id: {}, exchange: {}", account.account_id, account.exchange.name);

    let (start_ts, end_ts) = resolve_time_range(params.start_ts, params.end_ts, params.period, params.period_type.clone()).await?;
    debug!("Using time range: start_ts={}, end_ts={}", start_ts, end_ts);

    let calc_method = if params.precise_comb.as_deref() == Some("1") {
        CalcMethod::Precise
    } else {
        CalcMethod::Coarse
    };

    let interval = match params.coarse_interval.as_deref() {
        Some("1d") => ChronoDuration::days(1),
        Some("7d") => ChronoDuration::days(7),
        Some("30d") => ChronoDuration::days(30),
        Some("90d") => ChronoDuration::days(90),
        Some("365d") => ChronoDuration::days(365),
        _ => {
            if calc_method == CalcMethod::Coarse {
                return Err(AppError::BadRequest("coarse_interval must be specified for coarse aggregation (e.g., 1d, 7d, 30d, 90d, 365d)".to_string()));
            }
            ChronoDuration::days(1) // Default for precise mode, not used
        }
    };

    let week_align = params.week_align.as_deref().map(|s| s == "1" || s.to_lowercase() == "true").unwrap_or(false);

    
    let result = timeout(Duration::from_secs(60), async {
        debug!("Creating TradesCache for account_id={}, pair_id={}", account.account_id, BTC_PAIR_ID);
        let trades_cache = account.get_trades_cache(BTC_PAIR_ID).await;

        debug!("Creating TradesAggregator with calc_method={:?}, week_align={}", calc_method, week_align);
        let mut aggregator = TradesAggregator::new(
            trades_cache,
            start_ts,
            end_ts,
            interval,
            calc_method.clone(),
            week_align,
            false, // No auto-registration in API context
        ).await;

        debug!("Running aggregation");
        if calc_method == CalcMethod::Coarse {
            aggregator.aggregate_coarse().await
                .map_err(|e| AppError::Internal(format!("Failed to aggregate trades: {}", e)))?;
        } else {
            aggregator.aggregate_precise().await
                .map_err(|e| AppError::Internal(format!("Failed to aggregate trades: {}", e)))?;
        }

        let trades = aggregator.results;
        debug!("Aggregation completed: {} virtual trades", trades.len());
        Ok(Json(trades))
    })
    .await;

    match result {
        Ok(Ok(json)) => {
            info!("Aggregated trades request completed: {} trades", json.0.len());
            Ok(json)
        }
        Ok(Err(e)) => {
            error!("Aggregated trades request failed: {:?}", e);
            Err(e)
        }
        Err(_) => {
            let backtrace = Backtrace::new();
            error!("Request timed out after 60 seconds\nBacktrace:\n{:?}", backtrace);
            Err(AppError::Internal(format!("Request timed out after 60 seconds\nBacktrace:\n{:?}", backtrace)))
        }
    }
}
