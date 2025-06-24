// /src/services/cache/price.rs
// Created: 2025-06-24 10:41:00 EEST

use chrono::{DateTime, Utc, Duration};
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{info, debug, error};
use async_trait::async_trait;
use delegate::delegate;

use crate::{
    db::mysql::MySqlDataSource,
    entities::public_data::{Candle, PublicDataSource},
    entities::cache::{PriceCache, LoadPriceCacheTask},
    entities::task::{Task, Status, TaskBase},
    entities::exchange::Exchange,
    logs::app_error::AppError,
    services::task_processor::TaskProcessor,
};

impl PriceCache {
    // Creates a new PriceCache instance for an exchange and pair
    pub fn new(exchange: Arc<Exchange>, pair_id: Option<i32>) -> Self {
        debug!("Creating new PriceCache for exchange={}, pair_id={:?}", exchange.name, pair_id);
        PriceCache {
            data: DashMap::new(),
            exchange,
            pair_id,
        }
    }

    // Prefetches VWAP prices for a time range with a one-day buffer
    pub async fn load_prefetch(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) -> Result<(), AppError> {
        let buffer = Duration::days(1);
        let prefetch_start = start - buffer;
        let prefetch_end = end + buffer;
        let db = MySqlDataSource::db_conn();

        debug!("Prefetching VWAP prices for exchange={}, pair_id={:?}, start={}, end={}", 
            self.exchange.name, self.pair_id, prefetch_start, prefetch_end);

        // Load candles for the buffered period
        let candles = db.load_candles(prefetch_start, prefetch_end, &self.exchange.name, self.pair_id)
            .await
            .map_err(|e| {
                error!("Failed to fetch candles: {}", e);
                AppError::Internal(format!("Failed to fetch candles: {}", e))
            })?;

        if candles.is_empty() {
            error!("No candles found for {} in range {} to {}", self.exchange.name, prefetch_start, prefetch_end);
            return Err(AppError::Internal(format!("No candles found for pair_id={:?}", self.pair_id)));
        }

        // Calculate VWAP for each hour
        let mut hourly_data: std::collections::HashMap<i32, (f32, f32)> = std::collections::HashMap::new();
        for candle in candles {
            let hour_timestamp = (candle.ts.timestamp() / 3600) as i32;
            let avg_price = (candle.open + candle.high + candle.low + candle.close) / 4.0;
            let price_volume = avg_price * candle.volume;

            let entry = hourly_data.entry(hour_timestamp).or_insert((0.0, 0.0));
            entry.0 += price_volume; // Total price * volume
            entry.1 += candle.volume; // Total volume
        }

        // Store VWAP in cache
        for (&hour_timestamp, &(total_price_volume, total_volume)) in &hourly_data {
            if total_volume > 0.0 {
                let vwap = total_price_volume / total_volume;
                self.data.insert(hour_timestamp, vwap);
                debug!("Cached VWAP={} for hour_timestamp={}", vwap, hour_timestamp);
            } else {
                error!("Zero volume for hour_timestamp={}", hour_timestamp);
            }
        }

        info!("Prefetched {} VWAP entries for exchange={}, pair_id={:?}", 
            hourly_data.len(), self.exchange.name, self.pair_id);
        Ok(())
    }

    // Retrieves VWAP price for a timestamp, prefetching data or using the last available VWAP if necessary
    pub async fn get_vwap(
        &self,
        timestamp: DateTime<Utc>,
    ) -> Result<f32, AppError> {
        let hour_timestamp = (timestamp.timestamp() / 3600) as i32;
        if let Some(price) = self.data.get(&hour_timestamp) {
            debug!("Cache hit for hour_timestamp={}", hour_timestamp);
            return Ok(*price);
        }

        debug!("Cache miss for hour_timestamp={}, prefetching data", hour_timestamp);
        self.load_prefetch(timestamp, timestamp)
            .await
            .map_err(|e| AppError::Internal(format!("Failed to prefetch VWAP: {}", e)))?;

        if let Some(price) = self.data.get(&hour_timestamp) {
            debug!("Retrieved VWAP={} for hour_timestamp={}", *price, hour_timestamp);
            return Ok(*price);
        }

        // Find the last available VWAP with a timestamp less than the requested
        let mut last_vwap = None;
        let mut max_timestamp = i32::MIN;
        for entry in self.data.iter() {
            let ts = *entry.key();
            if ts < hour_timestamp && ts > max_timestamp {
                max_timestamp = ts;
                last_vwap = Some(*entry.value());
            }
        }

        match last_vwap {
            Some(price) => {
                debug!("Using last available VWAP={} for hour_timestamp={}", price, max_timestamp);
                Ok(price)
            }
            None => {
                error!("No VWAP data available for timestamp {}", timestamp);
                Err(AppError::Internal(format!("No VWAP data available for timestamp {}", timestamp)))
            }
        }
    }
}

#[async_trait]
impl Task for LoadPriceCacheTask {
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
        debug!("Initializing LoadPriceCacheTask for exchange={}, pair_id={:?}", 
            self.cache.exchange.name, self.cache.pair_id);
        Ok(())
    }

    // Executes the task, loading price cache data
    async fn run(&mut self) -> Result<Status, String> {
        debug!("Running LoadPriceCacheTask for exchange={}, pair_id={:?}", 
            self.cache.exchange.name, self.cache.pair_id);

        match self.cache.load_prefetch(self.start_ts, self.end_ts).await {
            Ok(()) => {
                info!("Successfully loaded price cache for exchange={}, pair_id={:?}", 
                    self.cache.exchange.name, self.cache.pair_id);
                self.set_result(serde_json::Value::String("Loaded successfully".to_string()));
                self.set_status(Status::Completed);
                Ok(Status::Completed)
            }
            Err(e) => {
                error!("Failed to load price cache: {}", e);
                self.set_result(serde_json::Value::String(format!("Failed: {}", e)));
                self.set_status(Status::Failed);
                Ok(Status::Failed)
            }
        }
    }

    // Releases resources
    async fn release(&mut self) -> Result<(), String> {
        debug!("Releasing LoadPriceCacheTask for exchange={}, pair_id={:?}", 
            self.cache.exchange.name, self.cache.pair_id);
        Ok(())
    }
}

impl LoadPriceCacheTask {
    // Creates a new LoadPriceCacheTask instance, optionally registering it in TaskProcessor
    pub async fn new(
        cache: Arc<PriceCache>,
        start_ts: DateTime<Utc>,
        end_ts: DateTime<Utc>,
        auto_reg: bool,
    ) -> Self {
        debug!("Creating LoadPriceCacheTask for exchange={}, pair_id={:?}", cache.exchange.name, cache.pair_id);
        let task = LoadPriceCacheTask {
            base: TaskBase::new(),
            cache: cache.clone(),
            start_ts,
            end_ts,
        };
        if auto_reg {
            debug!("Auto-registering LoadPriceCacheTask");
            if let Err(e) = task.base.self_reg(task.clone()).await {
                error!("Failed to auto-register LoadPriceCacheTask: {}", e);
            } else {
                info!("Successfully auto-registered LoadPriceCacheTask for exchange={}, pair_id={:?}", 
                    cache.clone().exchange.name, cache.clone().pair_id);
            }
        }
        task
    }
}