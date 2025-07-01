use async_trait::async_trait;
use chrono::{DateTime, Utc, Duration};
use serde_json::{to_value};
use std::sync::Arc;
use tracing::{debug, error, info};
use delegate::delegate;

use crate::{
    entities::trade::Trade,
    entities::cache::TradesCache,
    entities::task::{Task, TaskStatus, TaskBase},
    logs::app_error::AppError,
    common::math::auto_round,
    common::interval_func::{
        adjust_to_monday, adjust_to_first_of_month, adjust_to_first_of_quarter, adjust_to_first_of_year,
        HOUR_SECONDS, DAY_SECONDS, WEEK_SECONDS, MONTH_SECONDS, QUARTER_SECONDS, YEAR_SECONDS,
    },
};

// Defines aggregation method for trades
#[derive(Debug, Clone, PartialEq)]
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
    pub week_align: bool,
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
            week_align: self.week_align,
            results: self.results.clone(),
        }
    }
}

impl TradesAggregator {
    pub async fn new(
        trades_cache: Arc<TradesCache>,
        start_ts: DateTime<Utc>,
        end_ts: DateTime<Utc>,
        interval: Duration,
        calc_method: CalcMethod,
        week_align: bool,
        auto_reg: bool,
    ) -> Self {
        debug!("#DBG: Creating TradesAggregator for pair_id={}, calc_method={:?}, week_align={}", 
            trades_cache.pair_id, calc_method, week_align);
        let task = TradesAggregator {
            base: TaskBase::new(),
            trades_cache,
            start_ts,
            end_ts,
            interval,
            calc_method,
            week_align,
            results: Vec::new(),
        };
        if auto_reg {
            debug!("#DBG: Auto-registering TradesAggregator");
            if let Err(e) = task.base.self_reg(task.clone()).await {
                error!("#ERROR: Failed to auto-register TradesAggregator: {}", e);
            } else {
                info!("#INFO: Auto-registered TradesAggregator for pair_id={}", 
                    task.trades_cache.pair_id);
            }
        }
        task
    }

    fn aggregate_trades(&self, trades: Vec<&Trade>, buy: bool, ts: DateTime<Utc>, pair_id: i32) -> Option<Trade> {
        if trades.is_empty() {
            return None;
        }
        let total_amount: f32 = trades.iter().map(|t| t.amount).sum();
        let total_price_volume: f32 = trades.iter().map(|t| t.price * t.amount).sum();
        let avg_price = if total_amount > 0.0 { 
            total_price_volume / total_amount
        } else { 
            0.0 
        };
        
        let rounded_amount = auto_round(total_amount, 0);
        let rounded_price = auto_round(avg_price, 0);

        let trade_type = if buy { "buy" } else { "sell" };
        for trade in &trades {
            debug!("#DBG: Source trade: trade_no={}, ts={}, price={}, amount={} in virtual {} trade at {}", 
                trade.trade_no, trade.ts, trade.price, trade.amount, trade_type, ts);
        }
        debug!("#DBG: input price {}, amount {}, output price {}, amount {}", 
            avg_price, total_amount, rounded_price, rounded_amount);

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
            order_id: 0,
            position: 0.0,
            rpnl: 0.0,
            flags: trades.len() as i32,
            comission: 0.0,
        })
    }

    pub async fn aggregate_coarse(&mut self) -> Result<(), AppError> {
        debug!("#DBG: Aggregating trades in coarse mode with interval={:?}, week_align={}", 
            self.interval, self.week_align);
        let mut trades = self.trades_cache.get_trades(self.start_ts, self.end_ts).await?;
        trades.sort_by(|a, b| a.ts.cmp(&b.ts));
        debug!("#DBG: Sorted {} trades", trades.len());

        let interval_seconds = self.interval.num_seconds();
        let is_weekly = interval_seconds == WEEK_SECONDS;
        let is_monthly = interval_seconds == MONTH_SECONDS;
        let is_quarterly = interval_seconds == QUARTER_SECONDS;
        let is_yearly = interval_seconds == YEAR_SECONDS;
        let mut buys: std::collections::HashMap<DateTime<Utc>, Vec<&Trade>> = std::collections::HashMap::new();
        let mut sells: std::collections::HashMap<DateTime<Utc>, Vec<&Trade>> = std::collections::HashMap::new();

        for trade in trades.iter() {
            let window_start = if is_yearly {
                adjust_to_first_of_year(trade.ts, self.week_align)
            } else if is_quarterly {
                adjust_to_first_of_quarter(trade.ts, self.week_align)
            } else if is_monthly {
                adjust_to_first_of_month(trade.ts, interval_seconds, self.week_align)
            } else if is_weekly {
                adjust_to_monday(trade.ts)
            } else {
                let seconds_since_start = (trade.ts.timestamp() - self.start_ts.timestamp()) / interval_seconds * interval_seconds;
                self.start_ts + Duration::seconds(seconds_since_start)
            };
            if trade.buy {
                buys.entry(window_start).or_insert_with(Vec::new).push(trade);
            } else {
                sells.entry(window_start).or_insert_with(Vec::new).push(trade);
            }
        }

        let mut window_starts: Vec<DateTime<Utc>> = buys.keys().chain(sells.keys()).cloned().collect();
        window_starts.sort();
        window_starts.dedup();

        for window_start in window_starts {
            if let Some(buy_trades) = buys.get(&window_start) {
                debug!("#DBG: Aggregating {} buy trades at {}", buy_trades.len(), window_start);
                if let Some(buy_trade) = self.aggregate_trades(buy_trades.clone(), true, window_start, self.trades_cache.pair_id) {
                    self.results.push(buy_trade);
                }
            }
            if let Some(sell_trades) = sells.get(&window_start) {
                debug!("#DBG: Aggregating {} sell trades at {}", sell_trades.len(), window_start);
                if let Some(sell_trade) = self.aggregate_trades(sell_trades.clone(), false, window_start, self.trades_cache.pair_id) {
                    self.results.push(sell_trade);
                }
            }
        }

        self.results.sort_by(|a, b| a.ts.cmp(&b.ts).then(a.buy.cmp(&b.buy).reverse()));
        debug!("#DBG: Sorted {} virtual trades", self.results.len());
        info!("#INFO: Coarse aggregation completed: {} virtual trades", self.results.len());
        Ok(())
    }

    pub async fn aggregate_precise(&mut self) -> Result<(), AppError> {
        debug!("#DBG: Aggregating trades in precise mode");
        let mut trades = self.trades_cache.get_trades(self.start_ts, self.end_ts).await?;
        
        trades.sort_by(|a, b| a.ts.cmp(&b.ts));
        debug!("#DBG: Sorted {} trades", trades.len());
        
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

        self.results.sort_by(|a, b| a.ts.cmp(&b.ts));
        debug!("#DBG: Sorted {} virtual trades", self.results.len());
        info!("#INFO: Precise aggregation completed: {} virtual trades", self.results.len());
        Ok(())
    }
}

#[async_trait]
impl Task for TradesAggregator {
    delegate! {
        to self.base {
            fn status(&self) -> TaskStatus;
            fn set_status(&mut self, status: TaskStatus);
            fn result(&self) -> serde_json::Value;
            fn set_result(&mut self, result: serde_json::Value);
            fn start_at(&self) -> DateTime<Utc>;
            fn set_start_at(&mut self, start_at: DateTime<Utc>);
            fn id(&self) -> u32;
            fn set_id(&mut self, id: u32);
        }
    }

    async fn init(&mut self) -> Result<(), String> {
        debug!("#DBG: Initializing TradesAggregator for pair_id={}", self.trades_cache.pair_id);
        Ok(())
    }

    async fn run(&mut self) -> Result<TaskStatus, String> {
        debug!("#DBG: Running TradesAggregator for pair_id={}, calc_method={:?}, week_align={}", 
            self.trades_cache.pair_id, self.calc_method, self.week_align);

        let result = match self.calc_method {
            CalcMethod::Coarse => self.aggregate_coarse().await,
            CalcMethod::Precise => self.aggregate_precise().await,
        };

        match result {
            Ok(()) => {
                info!("#INFO: Aggregated {} trades for pair_id={}", 
                    self.results.len(), self.trades_cache.pair_id);                
                self.set_result(to_value(self.results.clone()).map_err(|e| format!("Failed to serialize trades: {}", e))?);
                self.set_status(TaskStatus::Completed);
                Ok(TaskStatus::Completed)
            }
            Err(e) => {
                error!("#ERROR: Failed to aggregate trades: {}", e);
                self.set_result(serde_json::Value::String(format!("Failed: {}", e)));
                self.set_status(TaskStatus::Failed);
                Ok(TaskStatus::Failed)
            }
        }
    }

    async fn release(&mut self) -> Result<(), String> {
        debug!("#DBG: Releasing TradesAggregator for pair_id={}", self.trades_cache.pair_id);
        self.results.clear();
        Ok(())
    }
}