// /src/common/math.rs
// Modified: 2025-06-24 13:15:00 EEST

use std::cmp::max;

/// Rounds a value based on its order of magnitude, with optional extra precision.
/// - `value`: The value to round.
/// - `extra_accuracy`: Additional decimal places to keep (default: 0).
/// Returns the rounded value.
pub fn auto_round(value: f64, extra_accuracy: i32) -> f64 {
    if value == 0.0 {
        return 0.0;
    }

    let abs_value = value.abs();
    let base_precision = if abs_value < 1.0 {
        5 // For values < 1, always use 5 decimal places
    } else {
        // For values >= 1, use 5 - floor(log10(|value|))
        max(0, 5 - abs_value.log10().floor() as i32)
    };
    let precision = base_precision + extra_accuracy;

    // Round to the calculated precision
    let factor = 10.0_f64.powi(precision);
    (value * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::auto_round;

    #[test]
    fn test_auto_round() {
        assert_eq!(auto_round(55073.767, 0), 55073.8); // >10000, 1 decimal
        assert_eq!(auto_round(4285.030, 1), 4285.030); // >1000, 2+1 decimals
        assert_eq!(auto_round(100000.5, 0), 100001.0); // >100000, 0 decimals
        assert_eq!(auto_round(100000.444, 0), 100000.0); // >100000, 0 decimals
        assert_eq!(auto_round(0.123456, 0), 0.12346); // <1, 5 decimals
        assert_eq!(auto_round(0.0, 0), 0.0); // Zero case
    }
}