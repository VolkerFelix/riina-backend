use std::io::Error;
use crate::models::workout_data::HeartRateData;

pub fn filter_heart_rate_data(heart_rate_data: &mut Vec<HeartRateData>) -> usize {
    if heart_rate_data.len() <= 1 {
        return 0;
    }
    let original_len = heart_rate_data.len();
    // Heart rate is already in ascending order
    // Remove samples if timestamp goes back in time compared to previous sample
    // Find the first point where this happens and truncate
    if let Some(pos) = heart_rate_data.windows(2).position(|w| w[1].timestamp <= w[0].timestamp) {
        // Truncate at that position, keeping only valid ascending data
        heart_rate_data.truncate(pos + 1);
    }

    original_len - heart_rate_data.len()
}