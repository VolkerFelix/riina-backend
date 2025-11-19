use crate::models::workout_data::HeartRateData;

pub fn filter_heart_rate_data(heart_rate_data: &mut Vec<HeartRateData>) -> usize {
    if heart_rate_data.len() <= 1 {
        return 0;
    }
    let original_len = heart_rate_data.len();

    // Remove duplicate or out-of-order timestamps while preserving valid data
    // Strategy: Keep samples where timestamp is strictly greater than previous
    let mut filtered_data = Vec::with_capacity(heart_rate_data.len());

    // Always keep the first sample
    filtered_data.push(heart_rate_data[0].clone());

    // For each subsequent sample, only keep if timestamp is strictly after the last kept sample
    for sample in heart_rate_data.iter().skip(1) {
        if sample.timestamp > filtered_data.last().unwrap().timestamp {
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