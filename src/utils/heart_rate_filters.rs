use crate::models::workout_data::HeartRateData;
use chrono::{DateTime, Utc};

pub fn filter_heart_rate_data(heart_rate_data: &mut Vec<HeartRateData>, workout_start: &DateTime<Utc>, workout_end: &DateTime<Utc>) -> usize {
    if heart_rate_data.is_empty() {
        return 0;
    }
    let original_len = heart_rate_data.len();

    // Remove samples outside workout time range, duplicates, or out-of-order timestamps
    // Strategy: Keep samples where timestamp is within range AND strictly greater than previous
    let mut filtered_data: Vec<HeartRateData> = Vec::with_capacity(heart_rate_data.len());

    for sample in heart_rate_data.iter() {
        // Check if sample is within workout time range
        if sample.timestamp < *workout_start || sample.timestamp > *workout_end {
            tracing::debug!(
                "Filtering out HR sample outside workout time range: {} (workout: {} to {})",
                sample.timestamp,
                workout_start,
                workout_end
            );
            continue;
        }

        // Check if timestamp is strictly after the last kept sample (or if this is the first sample)
        if filtered_data.is_empty() || sample.timestamp > filtered_data.last().unwrap().timestamp {
            filtered_data.push(sample.clone());
        } else {
            tracing::debug!(
                "Filtering out HR sample with non-increasing timestamp: {} <= {}",
                sample.timestamp,
                filtered_data.last().unwrap().timestamp
            );
        }
    }

    *heart_rate_data = filtered_data;
    original_len - heart_rate_data.len()
}