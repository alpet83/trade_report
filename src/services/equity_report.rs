// /src/services/equity_report.rs
// Modified: 2025-06-22 10:53:00 EEST

use chrono::{DateTime, Utc};
use chrono::Timelike;
use plotters::prelude::*;
use tracing::{info, debug};

use crate::entities::trade::Trade;
use crate::{
    db::mysql::MySqlDataSource,
    db::load_equity_data::LoadEquityData,
    entities::account::TradingAccount,
    entities::trade_data::TradeDataSource,
    services::chart::ChartReportGenerator,
};

pub struct EquityReportGenerator {    
    width: u32,
    height: u32,
}

impl EquityReportGenerator {
    // Creates a new equity report generator with specified dimensions
    pub fn new(width: u32, height: u32) -> Self {
        EquityReportGenerator { width, height }
    }
}

impl ChartReportGenerator for EquityReportGenerator {
    // Generates an SVG equity chart with semi-transparent fill, optional dark theme, and period-based aggregation
    async fn generate_svg(
        &self,
        account: &TradingAccount,
        start_ts: DateTime<Utc>,
        end_ts: DateTime<Utc>,
        value_column: Option<&str>,
        dark: bool,
        period_type: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let value_column = value_column.unwrap_or("value");
        info!("Generating SVG equity chart for account_id={} on {}, value_column={}, dark={}, period_type={:?}", 
            account.account_id, account.exchange.name, value_column, dark, period_type);

        let db = MySqlDataSource::db_conn();

        let mut equity_points = db.load_equity_data(
            account,            
            start_ts,
            end_ts,
            value_column,
        )
        .await
        .map_err(|e| format!("Failed to load equity data: {}", e))?;

        if equity_points.is_empty() {
            return Err("No equity data found".to_string());
        }

        // Aggregate points for weekly period (every 4 hours)
        if period_type == Some("weekly") {
            let mut aggregated_points = Vec::new();
            let interval = chrono::Duration::hours(4);
            let mut current_ts = start_ts.with_hour(start_ts.hour() / 4 * 4)
                .expect("Invalid datetime")
                .with_minute(0)
                .expect("Invalid datetime")
                .with_second(0)
                .expect("Invalid datetime")
                .with_nanosecond(0)
                .expect("Invalid datetime");

            while current_ts <= end_ts {
                let window_end = current_ts + interval;
                let window_points: Vec<_> = equity_points.iter()
                    .filter(|(ts, _)| *ts >= current_ts && *ts < window_end)
                    .collect();
                
                if !window_points.is_empty() {
                    let avg_value = window_points.iter().map(|(_, v)| *v).sum::<f32>() / window_points.len() as f32;
                    aggregated_points.push((current_ts, avg_value));
                }
                
                current_ts = window_end;
            }
            equity_points = aggregated_points;

            // Apply EMA smoothing for weekly period
            let alpha = 0.2;
            let mut smoothed_points = vec![(equity_points[0].0, equity_points[0].1)];
            for i in 1..equity_points.len() {
                let prev_ema = smoothed_points[i - 1].1;
                let current_value = equity_points[i].1;
                let ema = prev_ema + (current_value - prev_ema) * alpha;
                smoothed_points.push((equity_points[i].0, ema));
            }
            equity_points = smoothed_points;

            // Limit to ~1460 points
            if equity_points.len() > 1460 {
                let step = equity_points.len() / 1460;
                equity_points = equity_points.into_iter()
                    .enumerate()
                    .filter(|(i, _)| i % step == 0)
                    .map(|(_, point)| point)
                    .collect();
            }
        }

        let mut svg_string = String::new();
        {
            let root = SVGBackend::with_string(&mut svg_string, (self.width, self.height)).into_drawing_area();
            
            // Set theme colors
            let (background, font_color) = if dark {
                (RGBColor(30, 30, 30), RGBColor(255, 255, 255)) // Dark gray background, white font
            } else {
                (WHITE, BLACK) // White background, black font
            };

            root.fill(&background).map_err(|e| format!("Failed to fill background: {}", e))?;

            let min_ts = equity_points.iter().map(|(ts, _)| *ts).min().unwrap();
            let max_ts = equity_points.iter().map(|(ts, _)| *ts).max().unwrap();
            let min_equity = equity_points.iter().map(|(_, v)| *v).fold(f32::INFINITY, f32::min);
            let max_equity = equity_points.iter().map(|(_, v)| *v).fold(f32::NEG_INFINITY, f32::max);

            let mut chart = ChartBuilder::on(&root)
                .caption("Equity Chart", ("sans-serif", 20).into_font().color(&font_color))
                .x_label_area_size(40)
                .y_label_area_size(0) // Remove left Y-axis
                .right_y_label_area_size(40) // Add right Y-axis
                .margin(10)
                .build_cartesian_2d(min_ts..max_ts, min_equity..max_equity)
                .map_err(|e| format!("Failed to build chart: {}", e))?;

            chart.configure_mesh()
                .x_labels(10)
                .y_labels(10)
                .y_label_offset(5)
                .y_label_formatter(&|v| format!("{:.2}", v))
                .x_label_style(("sans-serif", 12).into_font().color(&font_color))
                .y_label_style(("sans-serif", 12).into_font().color(&font_color))
                .draw()
                .map_err(|e| format!("Failed to draw mesh: {}", e))?;

            chart.draw_series(AreaSeries::new(
                equity_points.iter().map(|(ts, v)| (*ts, *v)),
                0.0, // Baseline at Y=0
                ShapeStyle::from(&RGBColor(0, 128, 0).mix(0.5)), // Green with 50% opacity
            )).map_err(|e| format!("Failed to draw positive series: {}", e))?;

            chart.draw_series(AreaSeries::new(
                equity_points.iter().filter(|(_, v)| *v < 0.0).map(|(ts, v)| (*ts, *v)),
                0.0, // Baseline at Y=0
                ShapeStyle::from(&RGBColor(255, 0, 0).mix(0.5)), // Red with 50% opacity
            )).map_err(|e| format!("Failed to draw negative series: {}", e))?;
        }

        debug!("SVG chart generated successfully");
        Ok(svg_string.into_bytes())
    }

    // Generates a PNG equity chart with semi-transparent fill, optional dark theme, and period-based aggregation
    async fn generate_image(
        &self,
        account: &TradingAccount,
        start_ts: DateTime<Utc>,
        end_ts: DateTime<Utc>,
        value_column: Option<&str>,
        dark: bool,
        period_type: Option<&str>,
    ) -> Result<Vec<u8>, String> {
        let value_column = value_column.unwrap_or("value");
        info!("Generating PNG equity chart for account_id={} on {}, value_column={}, dark={}, period_type={:?}", 
            account.account_id, account.exchange.name, value_column, dark, period_type);

        let db = MySqlDataSource::db_conn();
        let mut equity_points = db.load_equity_data(
            account,
            start_ts,
            end_ts,
            value_column,
        )
        .await
        .map_err(|e| format!("Failed to load equity data: {}", e))?;

        if equity_points.is_empty() {
            return Err("No equity data found".to_string());
        }

        // Aggregate points for weekly period (every 4 hours)
        if period_type == Some("weekly") {
            let mut aggregated_points = Vec::new();
            let interval = chrono::Duration::hours(4);
            let mut current_ts = start_ts.with_hour(start_ts.hour() / 4 * 4)
                .expect("Invalid datetime")
                .with_minute(0)
                .expect("Invalid datetime")
                .with_second(0)
                .expect("Invalid datetime")
                .with_nanosecond(0)
                .expect("Invalid datetime");

            while current_ts <= end_ts {
                let window_end = current_ts + interval;
                let window_points: Vec<_> = equity_points.iter()
                    .filter(|(ts, _)| *ts >= current_ts && *ts < window_end)
                    .collect();
                
                if !window_points.is_empty() {
                    let avg_value = window_points.iter().map(|(_, v)| *v).sum::<f32>() / window_points.len() as f32;
                    aggregated_points.push((current_ts, avg_value));
                }
                
                current_ts = window_end;
            }
            equity_points = aggregated_points;

            // Apply EMA smoothing for weekly period
            let alpha = 0.2;
            let mut smoothed_points = vec![(equity_points[0].0, equity_points[0].1)];
            for i in 1..equity_points.len() {
                let prev_ema = smoothed_points[i - 1].1;
                let current_value = equity_points[i].1;
                let ema = prev_ema + (current_value - prev_ema) * alpha;
                smoothed_points.push((equity_points[i].0, ema));
            }
            equity_points = smoothed_points;

            // Limit to ~1460 points
            if equity_points.len() > 1460 {
                let step = equity_points.len() / 1460;
                equity_points = equity_points.into_iter()
                    .enumerate()
                    .filter(|(i, _)| i % step == 0)
                    .map(|(_, point)| point)
                    .collect();
            }
        }

        let mut buffer = Vec::new();
        {
            let root = BitMapBackend::with_buffer(&mut buffer, (self.width, self.height)).into_drawing_area();
            
            // Set theme colors
            let (background, font_color) = if dark {
                (RGBColor(30, 30, 30), RGBColor(255, 255, 255)) // Dark gray background, white font
            } else {
                (WHITE, BLACK) // White background, black font
            };

            root.fill(&background).map_err(|e| format!("Failed to fill background: {}", e))?;

            let min_ts = equity_points.iter().map(|(ts, _)| *ts).min().unwrap();
            let max_ts = equity_points.iter().map(|(ts, _)| *ts).max().unwrap();
            let min_equity = equity_points.iter().map(|(_, v)| *v).fold(f32::INFINITY, f32::min);
            let max_equity = equity_points.iter().map(|(_, v)| *v).fold(f32::NEG_INFINITY, f32::max);

            let mut chart = ChartBuilder::on(&root)
                .caption("Equity Chart", ("sans-serif", 20).into_font().color(&font_color))
                .x_label_area_size(40)
                .y_label_area_size(0) // Remove left Y-axis
                .right_y_label_area_size(40) // Add right Y-axis
                .margin(10)
                .build_cartesian_2d(min_ts..max_ts, min_equity..max_equity)
                .map_err(|e| format!("Failed to build chart: {}", e))?;

            chart.configure_mesh()
                .x_labels(10)
                .y_labels(10)
                .y_label_offset(5)
                .y_label_formatter(&|v| format!("{:.2}", v))
                .x_label_style(("sans-serif", 12).into_font().color(&font_color))
                .y_label_style(("sans-serif", 12).into_font().color(&font_color))
                .draw()
                .map_err(|e| format!("Failed to draw mesh: {}", e))?;

            chart.draw_series(AreaSeries::new(
                equity_points.iter().map(|(ts, v)| (*ts, *v)),
                0.0, // Baseline at Y=0
                ShapeStyle::from(&RGBColor(0, 128, 0).mix(0.5)), // Green with 50% opacity
            )).map_err(|e| format!("Failed to draw positive series: {}", e))?;

            chart.draw_series(AreaSeries::new(
                equity_points.iter().filter(|(_, v)| *v < 0.0).map(|(ts, v)| (*ts, *v)),
                0.0, // Baseline at Y=0
                ShapeStyle::from(&RGBColor(255, 0, 0).mix(0.5)), // Red with 50% opacity
            )).map_err(|e| format!("Failed to draw negative series: {}", e))?;
        }

        debug!("PNG chart generated successfully");
        Ok(buffer)
    }
}