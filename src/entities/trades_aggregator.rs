// /src/entities/trades_aggregator.rs
// Modified: 2025-06-24 14:00:00 EEST

use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration, Timelike};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, error, info};
use delegate::delegate;

use crate::{
    entities::trade::Trade,
    entities::cache::TradesCache,
    entities::task::{Task, Status, TaskBase},
    logs::app_error::AppError,
    common::math::auto_round,
};

// Defines aggregation method for trades
#[derive(Debug, Clone)]
pub enum CalcMethod {
    Coarse,
    Precise,
}

// Aggregates trades into virtual trades based on time windows or direction changes
#[derive(Debug)]
pub struct TradesAggregator {
    pub base: TaskBase,
    pub trades_cache: Arc<TradesCache>,
    pub start_ts: DateTime<Utc>,
    pub end_ts: DateTime<Utc>,
    pub interval: Duration,
    pub calc_method: CalcMethod,
    pub results: Vec<Trade>,
}

impl Clone for TradesAggregator {
    fn clone(&self) -> Self {
        TradesAggregator {
            base: self.base.clone(),
            trades_cache: Arc::clone(&self.trades_cache),
            start_ts: self.start_ts,
            end_ts: self.end_ts,
            interval: self.interval,
            calc_method: self.calc_method.clone(),
            results: self.results.clone(),
        }
    }
}

impl TradesAggregator {
    // Creates a new TradesAggregator instance, optionally registering it in TaskProcessor
    pub async fn new(
        trades_cache: Arc<TradesCache>,
        start_ts: DateTime<Utc>,
        end_ts: DateTime<Utc>,
        interval: Duration,
        calc_method: CalcMethod,
        auto_reg: bool,
    ) -> Self {
        debug!("#DBG: Creating TradesAggregator for pair_id={}, calc_method={:?}", 
            trades_cache.pair_id, calc_method);
        let task = TradesAggregator {
            base: TaskBase::new(),
            trades_cache,
            start_ts,
            end_ts,
            interval,
            calc_method,
            results: Vec::new(),
        };
        if auto_reg {
            debug!("#DBG: Auto-registering TradesAggregator");
            if let Err(e) = task.base.self_reg(task.clone()).await {
                error!("#ERROR: Failed to auto-register TradesAggregator: {}", e);
            } else {
                info!("#INFO: Successfully auto-registered TradesAggregator for pair_id={}", 
                    task.trades_cache.pair_id);
            }
        }
        task
    }

    // Aggregates a set of trades into a single virtual trade
    fn aggregate_trades(&self, trades: Vec<&Trade>, buy: bool, ts: DateTime<Utc>, pair_id: i32) -> Option<Trade> {
        if trades.is_empty() {
            return None;
        }
        let total_amount: f64 = trades.iter().map(|t| t.amount).sum();
        let total_price_volume: f64 = trades.iter().map(|t| t.price * t.amount).sum();
        let avg_price = if total_amount > 0.0 { 
            total_price_volume / total_amount
        } else { 
            0.0 
        };
        
        // Apply auto_round to final values
        let rounded_amount = auto_round(total_amount, 0);
        let rounded_price = auto_round(avg_price, 0);

        // Debug log for each source trade
        let trade_type = if buy { "buy" } else { "sell" };
        for trade in &trades {
            debug!("#DBG: Source trade: trade_no={}, ts={}, price={}, amount={} included in virtual {} trade at {}", 
                trade.trade_no, trade.ts, trade.price, trade.amount, trade_type, ts);
        }
        debug!("#DBG: input price {}, amount {}, production price {}, amount {}", 
            avg_price, total_amount, rounded_price, rounded_amount);

        // Form trade_no with first and last trade_no
        let trade_no = if trades.len() == 1 {
            trades[0].trade_no.clone()
        } else {
            format!("{}:{}", trades[0].trade_no, trades[trades.len() - 1].trade_no)
        };

        Some(Trade {
            ts,
            pair_id,
            buy,
            price: rounded_price,
            amount: rounded_amount,
            trade_no,
            order_id: "".to_string(),
            position: 0.0,
            rpnl: 0.0,
            flags: trades.len() as i32, // Store number of source trades
            comission: 0.0,
        })
    }

    // Aggregates trades using coarse method (fixed time intervals)
    pub async fn aggregate_coarse(&mut self) -> Result<(), AppError> {
        debug!("#DBG: Aggregating trades in coarse mode with interval={:?}", self.interval);
        let trades = self.trades_cache.get_trades(self.start_ts, self.end_ts).await?;
        
        let mut current_ts = self.start_ts;
        while current_ts < self.end_ts {
            let window_end = current_ts + self.interval;
            let current_date = current_ts.date_naive();
            let window_trades: Vec<&Trade> = trades.iter()
                .filter(|t| t.ts.date_naive() == current_date)
                .collect();

            // Aggregate buys
            let buys: Vec<&Trade> = window_trades.iter()
                .filter(|t| t.buy)
                .cloned()
                .collect();
            if let Some(buy_trade) = self.aggregate_trades(buys, true, current_ts, self.trades_cache.pair_id) {
                self.results.push(buy_trade);
            }

            // Aggregate sells
            let sells: Vec<&Trade> = window_trades.iter()
                .filter(|t| !t.buy)
                .cloned()
                .collect();
            if let Some(sell_trade) = self.aggregate_trades(sells, false, current_ts, self.trades_cache.pair_id) {
                self.results.push(sell_trade);
            }

            current_ts = window_end;
        }

        info!("#INFO: Coarse aggregation completed: {} virtual trades", self.results.len());
        Ok(())
    }

    // Aggregates trades using precise method (window closes on direction change)
    pub async fn aggregate_precise(&mut self) -> Result<(), AppError> {
        debug!("#DBG: Aggregating trades in precise mode");
        let trades = self.trades_cache.get_trades(self.start_ts, self.end_ts).await?;
        
        let mut current_trades: Vec<&Trade> = Vec::new();
        let mut current_direction: Option<bool> = None;

        for trade in trades.iter() {
            let trade_direction = trade.buy;
            if current_direction.is_none() {
                current_direction = Some(trade_direction);
                current_trades.push(trade);
            } else if current_direction == Some(trade_direction) {
                current_trades.push(trade);
            } else {
                // Direction changed, close current window
                if let Some(virtual_trade) = self.aggregate_trades(
                    current_trades.clone(),
                    current_direction.unwrap(),
                    current_trades[0].ts,
                    self.trades_cache.pair_id
                ) {
                    self.results.push(virtual_trade);
                }
                current_trades.clear();
                current_trades.push(trade);
                current_direction = Some(trade_direction);
            }
        }

        // Close final window
        if !current_trades.is_empty() {
            if let Some(virtual_trade) = self.aggregate_trades(
                current_trades.clone(),
                current_direction.unwrap(),
                current_trades[0].ts,
                self.trades_cache.pair_id
            ) {
                self.results.push(virtual_trade);
            }
        }

        info!("#INFO: Precise aggregation completed: {} virtual trades", self.results.len());
        Ok(())
    }
}

#[async_trait]
impl Task for TradesAggregator {
    delegate! {
        to self.base {
            fn status(&self) -> Status;
            fn set_status(&mut self, status: Status);
            fn result(&self) -> serde_json::Value;
            fn set_result(&mut self, result: serde_json::Value);
            fn start_at(&self) -> DateTime<Utc>;
            fn set_start_at(&mut self, start_at: DateTime<Utc>);
            fn id(&self) -> u32;
            fn set_id(&mut self, id: u32);
        }
    }

    // Initializes the task
    async fn init(&mut self) -> Result<(), String> {
        debug!("#DBG: Initializing TradesAggregator for pair_id={}", self.trades_cache.pair_id);
        Ok(())
    }

    // Executes the task, aggregating trades
    async fn run(&mut self) -> Result<Status, String> {
        debug!("#DBG: Running TradesAggregator for pair_id={}, calc_method={:?}", 
            self.trades_cache.pair_id, self.calc_method);

        let result = match self.calc_method {
            CalcMethod::Coarse => self.aggregate_coarse().await,
            CalcMethod::Precise => self.aggregate_precise().await,
        };

        match result {
            Ok(()) => {
                info!("#INFO: Successfully aggregated {} trades for pair_id={}", 
                    self.results.len(), self.trades_cache.pair_id);
                self.set_result(serde_json::Value::String(format!("Aggregated {} trades", self.results.len())));
                self.set_status(Status::Completed);
                Ok(Status::Completed)
            }
            Err(e) => {
                error!("#ERROR: Failed to aggregate trades: {}", e);
                self.set_result(serde_json::Value::String(format!("Failed: {}", e)));
                self.set_status(Status::Failed);
                Ok(Status::Failed)
            }
        }
    }

    // Releases resources
    async fn release(&mut self) -> Result<(), String> {
        debug!("#DBG: Releasing TradesAggregator for pair_id={}", self.trades_cache.pair_id);
        self.results.clear();
        Ok(())
    }
}