// Modified: 2025-06-19 15:50:00 EEST
// xaiArtifact: artifact_id="4e4be803-f66f-41eb-bce3-f08261d97f41", artifact_version_id="8d9e0f1a-2b3c-4d5e-6f7a-8b9c0d1e2f3a"

use chrono::{DateTime, Utc};
use crate::entities::account::TradingAccount;
use std::future::Future;

pub trait ChartReportGenerator {
    fn generate_svg(
        &self,
        account: &TradingAccount,
        start_ts: DateTime<Utc>,
        end_ts: DateTime<Utc>,
        value_column: Option<&str>,
    ) -> impl Future<Output = Result<Vec<u8>, String>>;

    fn generate_image(
        &self,
        account: &TradingAccount,
        start_ts: DateTime<Utc>,
        end_ts: DateTime<Utc>,
        value_column: Option<&str>,
    ) -> impl Future<Output = Result<Vec<u8>, String>>;
}