use axum::{Router, routing::get, Json, extract::Query, http::{Response, HeaderMap, StatusCode}};
use serde_json::{Map, Value};
use tokio::time::{timeout, Duration};
use tracing::{info, error, debug};
use backtrace::Backtrace;
use serde::Deserialize;
use chrono::{DateTime, Utc, Timelike};

use crate::{    
    entities::account::{TradingAccount, get_account_manager},
    services::deposit_basic_report::{DepositBasicReport, generate_deposit_report},
    services::{chart::ChartReportGenerator, equity_report::EquityReportGenerator},
    db::mysql::MySqlDataSource,
    config::Config,    
    logs::app_error::AppError,
};

#[derive(Deserialize)]
pub struct DepositReportQuery {
    exchange: Option<String>,
    account_id: Option<String>,
    applicant: Option<String>,
    period: Option<i64>,
    value_column: Option<String>,
    start_ts: Option<String>,
    end_ts: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}

pub fn routes() -> Router {
    Router::new()
        .route("/accounts", get(get_accounts))
    //   .route("/deposit_report", get(get_deposit_report))
    //   .route("/equity_chart", get(get_equity_chart))
}

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

async fn get_deposit_report(Query(params): Query<DepositReportQuery>) -> Result<Json<DepositBasicReport>, AppError> {
    info!("Starting deposit report request");

    debug!("Validating query parameters");
    let account = match (params.exchange, params.account_id, params.applicant) {
        (Some(exchange), Some(account_id), None) => {
            let manager = get_account_manager();
            let guard = manager.read().await;
            let acc = guard.get_account(&account_id)
                .filter(|acc| acc.exchange.name.to_lowercase() == exchange.to_lowercase())
                .ok_or_else(|| AppError::Internal(format!("No account found for account_id={} on exchange={}", account_id, exchange)))?;
            acc.clone()
        }
        (None, None, Some(applicant)) => {
            let manager = get_account_manager();
            let guard = manager.read().await;
            let acc = guard.find_account(&applicant)
                .ok_or_else(|| AppError::Internal(format!("No account found for applicant: {}", applicant)))?;
            acc.clone()
        }
        _ => return Err(AppError::Internal("Must provide either (exchange, account_id) or applicant".to_string())),
    };

    debug!("Selected account_id: {}, exchange: {}", account.account_id, account.exchange.name);

    let end_ts = match params.end_ts {
        Some(end) => DateTime::parse_from_rfc3339(&end)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| AppError::Internal(format!("Invalid end_ts format: {}", e)))?,
        None => Utc::now(),
    };

    let period = params.period.unwrap_or(24);
    let start_ts = match params.start_ts {
        Some(start) => DateTime::parse_from_rfc3339(&start)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| AppError::Internal(format!("Invalid start_ts format: {}", e)))?,
        None => (end_ts - chrono::Duration::hours(period))
            .with_minute(0)
            .expect("Invalid datetime")
            .with_second(0)
            .expect("Invalid datetime")
            .with_nanosecond(0)
            .expect("Invalid datetime"),
    };

    debug!("Using time range: start_ts={}, end_ts={}", start_ts, end_ts);

    let result = timeout(Duration::from_secs(60), async {
        debug!("Creating MySqlDataSource");
        let config = Config::load().map_err(|e| AppError::Internal(format!("Failed to load config: {}", e)))?;
        let db = MySqlDataSource::new(&config.mysql.url)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to connect to DB: {}", e)))?;

        debug!("Generating deposit report for account_id={} on {}", account.account_id, account.exchange.name);
        let report = generate_deposit_report(&db, &account, start_ts, end_ts, params.value_column.as_deref())
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

async fn get_equity_chart(Query(params): Query<DepositReportQuery>) -> Result<axum::http::Response<String>, AppError> {
    info!("Starting equity chart request");

    debug!("Validating query parameters");
    let account = match (params.exchange, params.account_id, params.applicant) {
        (Some(exchange), Some(account_id), None) => {
            let manager = get_account_manager();
            let guard = manager.read().await;
            let acc = guard.get_account(&account_id)
                .filter(|acc| acc.exchange.name.to_lowercase() == exchange.to_lowercase())
                .ok_or_else(|| AppError::Internal(format!("No account found for account_id={} on exchange={}", account_id, exchange)))?;
            acc.clone()
        }
        (None, None, Some(applicant)) => {
            let manager = get_account_manager();
            let guard = manager.read().await;
            let acc = guard.find_account(&applicant)
                .ok_or_else(|| AppError::Internal(format!("No account found for applicant: {}", applicant)))?;
            acc.clone()
        }
        _ => return Err(AppError::Internal("Must provide either (exchange, account_id) or applicant".to_string())),
    };

    debug!("Selected account_id: {}, exchange: {}", account.account_id, account.exchange.name);

    let end_ts = match params.end_ts {
        Some(end) => DateTime::parse_from_rfc3339(&end)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| AppError::Internal(format!("Invalid end_ts format: {}", e)))?,
        None => Utc::now(),
    };

    let period = params.period.unwrap_or(24);
    let start_ts = match params.start_ts {
        Some(start) => DateTime::parse_from_rfc3339(&start)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| AppError::Internal(format!("Invalid start_ts format: {}", e)))?,
        None => (end_ts - chrono::Duration::hours(period))
            .with_minute(0)
            .expect("Invalid datetime")
            .with_second(0)
            .expect("Invalid datetime")
            .with_nanosecond(0)
            .expect("Invalid datetime"),
    };

    let width = params.width.unwrap_or(800);
    let height = params.height.unwrap_or(600);

    debug!("Using time range: start_ts={}, end_ts={}, width={}, height={}", start_ts, end_ts, width, height);

    let result = timeout(Duration::from_secs(60), async {
        debug!("Creating MySqlDataSource");
        let config = Config::load().map_err(|e| AppError::Internal(format!("Failed to load config: {}", e)))?;
        let db = MySqlDataSource::new(&config.mysql.url)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to connect to DB: {}", e)))?;

        debug!("Generating equity chart for account_id={} on {}", account.account_id, account.exchange.name);
        let generator = EquityReportGenerator::new(&db, width, height);
        let svg_data = generator.generate_svg(&account, start_ts, end_ts, params.value_column.as_deref())
            .await
            .map_err(|e| AppError::Internal(format!("Failed to generate chart: {}", e)))?;

        debug!("Chart generated successfully");
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", "image/svg+xml".parse().unwrap());
        Ok(axum::http::Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "image/svg+xml")
            .body(String::from_utf8(svg_data).unwrap()) // Изменено: Vec<u8> -> String
            .unwrap())
    })
    .await;

    match result {
        Ok(Ok(response)) => {
            info!("Equity chart request completed");
            Ok(response)
        }
        Ok(Err(e)) => {
            error!("Equity chart request failed: {:?}", e);
            Err(e)
        }
        Err(_) => {
            let backtrace = Backtrace::new();
            error!("Request timed out after 60 seconds\nBacktrace:\n{:?}", backtrace);
            Err(AppError::Internal(format!("Request timed out after 60 seconds\nBacktrace:\n{:?}", backtrace)))
        }
    }
}