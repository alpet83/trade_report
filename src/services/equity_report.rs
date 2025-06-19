// Modified: 2025-06-19 15:55:00 EEST
// xaiArtifact: artifact_id="e2785c5c-0460-4ad7-87c4-12d65b7dd80e", artifact_version_id="1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d"

use chrono::{DateTime, Utc};
use plotters::prelude::*;
use tracing::{info, debug};

use crate::{
    entities::account::TradingAccount,
    db::mysql::TradeDataSource,
    services::chart::ChartReportGenerator,
};

pub struct EquityReportGenerator<'a> {
    db: &'a dyn TradeDataSource,
    width: u32,
    height: u32,
}

impl<'a> EquityReportGenerator<'a> {
    pub fn new(db: &'a dyn TradeDataSource, width: u32, height: u32) -> Self {
        EquityReportGenerator { db, width, height }
    }
}

impl<'a> ChartReportGenerator for EquityReportGenerator<'a> {
    async fn generate_svg(
        &self,
        account: &TradingAccount,
        start_ts: DateTime<Utc>,
        end_ts: DateTime<Utc>,
        value_column: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let value_column = value_column.unwrap_or("value");
        info!("Generating SVG equity chart for account_id={} on {}, value_column={}", account.account_id, account.exchange.name, value_column);

        let equity_points = self.db.load_equity_data(
            &account.exchange.name.to_lowercase(),
            account.account_id.parse::<i32>().map_err(|e| format!("Invalid account_id: {}", e))?,
            start_ts,
            end_ts,
            value_column,
        )
        .await
        .map_err(|e| format!("Failed to load equity data: {}", e))?;

        if equity_points.is_empty() {
            return Err("No equity data found".to_string());
        }

        let mut svg_string = String::new();
        {
            let root = SVGBackend::with_string(&mut svg_string, (self.width, self.height)).into_drawing_area();
            root.fill(&WHITE).map_err(|e| format!("Failed to fill background: {}", e))?;

            let min_ts = equity_points.iter().map(|(ts, _)| *ts).min().unwrap();
            let max_ts = equity_points.iter().map(|(ts, _)| *ts).max().unwrap();
            let min_equity = equity_points.iter().map(|(_, v)| *v).fold(f32::INFINITY, f32::min);
            let max_equity = equity_points.iter().map(|(_, v)| *v).fold(f32::NEG_INFINITY, f32::max);

            let mut chart = ChartBuilder::on(&root)
                .caption("Equity Chart", ("sans-serif", 20))
                .x_label_area_size(40)
                .y_label_area_size(40)
                .margin(10)
                .build_cartesian_2d(min_ts..max_ts, min_equity..max_equity)
                .map_err(|e| format!("Failed to build chart: {}", e))?;

            chart.configure_mesh().draw().map_err(|e| format!("Failed to draw mesh: {}", e))?;

            chart.draw_series(LineSeries::new(
                equity_points.iter().map(|(ts, v)| (*ts, *v)),
                ShapeStyle::from(&RGBColor(0, 128, 0)).stroke_width(2), // Green for positive
            )).map_err(|e| format!("Failed to draw series: {}", e))?;

            chart.draw_series(LineSeries::new(
                equity_points.iter().filter(|(_, v)| *v < 0.0).map(|(ts, v)| (*ts, *v)),
                ShapeStyle::from(&RGBColor(255, 0, 0)).stroke_width(2), // Red for negative
            )).map_err(|e| format!("Failed to draw negative series: {}", e))?;
        }

        debug!("SVG chart generated successfully");
        Ok(svg_string.into_bytes())
    }

    async fn generate_image(
        &self,
        account: &TradingAccount,
        start_ts: DateTime<Utc>,
        end_ts: DateTime<Utc>,
        value_column: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let value_column = value_column.unwrap_or("value");
        info!("Generating PNG equity chart for account_id={} on {}, value_column={}", account.account_id, account.exchange.name, value_column);

        let equity_points = self.db.load_equity_data(
            &account.exchange.name.to_lowercase(),
            account.account_id.parse::<i32>().map_err(|e| format!("Invalid account_id: {}", e))?,
            start_ts,
            end_ts,
            value_column,
        )
        .await
        .map_err(|e| format!("Failed to load equity data: {}", e))?;

        if equity_points.is_empty() {
            return Err("No equity data found".to_string());
        }

        let mut buffer = Vec::new();
        {
            let root = BitMapBackend::with_buffer(&mut buffer, (self.width, self.height)).into_drawing_area();
            root.fill(&WHITE).map_err(|e| format!("Failed to fill background: {}", e))?;

            let min_ts = equity_points.iter().map(|(ts, _)| *ts).min().unwrap();
            let max_ts = equity_points.iter().map(|(ts, _)| *ts).max().unwrap();
            let min_equity = equity_points.iter().map(|(_, v)| *v).fold(f32::INFINITY, f32::min);
            let max_equity = equity_points.iter().map(|(_, v)| *v).fold(f32::NEG_INFINITY, f32::max);

            let mut chart = ChartBuilder::on(&root)
                .caption("Equity Chart", ("sans-serif", 20))
                .x_label_area_size(40)
                .y_label_area_size(40)
                .margin(10)
                .build_cartesian_2d(min_ts..max_ts, min_equity..max_equity)
                .map_err(|e| format!("Failed to build chart: {}", e))?;

            chart.configure_mesh().draw().map_err(|e| format!("Failed to draw mesh: {}", e))?;

            chart.draw_series(LineSeries::new(
                equity_points.iter().map(|(ts, v)| (*ts, *v)),
                ShapeStyle::from(&RGBColor(0, 128, 0)).stroke_width(2), // Green for positive
            )).map_err(|e| format!("Failed to draw series: {}", e))?;

            chart.draw_series(LineSeries::new(
                equity_points.iter().filter(|(_, v)| *v < 0.0).map(|(ts, v)| (*ts, *v)),
                ShapeStyle::from(&RGBColor(255, 0, 0)).stroke_width(2), // Red for negative
            )).map_err(|e| format!("Failed to draw negative series: {}", e))?;
        }

        debug!("PNG chart generated successfully");
        Ok(buffer)
    }
}