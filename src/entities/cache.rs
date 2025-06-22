// /src/entities/cache.rs
// Modified: 2025-06-22 09:45:00 EEST

use chrono::{DateTime, Utc, Duration};
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{info, debug, error};

use crate::{
    db::mysql::MySqlDataSource,
    entities::public_data::{Candle, PublicDataSource},
    entities::exchange::Exchange,
    logs::app_error::AppError,
};

// Caches VWAP prices for an exchange and pair
#[derive(Debug)]
pub struct PriceCache {
    pub data: DashMap<i32, f32>,
    pub exchange: Arc<Exchange>,
    pub pair_id: Option<i32>,
}

impl PriceCache {
    // Creates a new PriceCache instance for an exchange and pair
    pub fn new(exchange: Arc<Exchange>, pair_id: Option<i32>) -> Self {
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
            .map_err(|e| AppError::Internal(format!("Failed to fetch candles: {}", e)))?;

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

    // Retrieves VWAP price for a timestamp, prefetching data if necessary
    pub async fn get_vwap(
        &self,
        db: &dyn PublicDataSource,
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

        self.data.get(&hour_timestamp)
            .map(|price| {
                debug!("Retrieved VWAP={} for hour_timestamp={}", *price, hour_timestamp);
                *price
            })
            .ok_or_else(|| {
                error!("No VWAP data for hour_timestamp={} after prefetch", hour_timestamp);
                AppError::Internal(format!("No VWAP data for timestamp {}", timestamp))
            })
    }
}