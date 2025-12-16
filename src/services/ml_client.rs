use serde::{Deserialize, Serialize};
use reqwest::Client;

#[derive(Debug, Serialize)]
pub struct HeartRateSample {
    pub timestamp: String,
    pub heart_rate: i32,
}

#[derive(Debug, Serialize)]
pub struct ClassifyRequest {
    pub heart_rate_data: Vec<HeartRateSample>,
    pub user_resting_hr: i32,
    pub user_max_hr: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activity_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ClassifyResponse {
    pub prediction: String,
    pub confidence: f64,
}

impl Default for ClassifyResponse {
    fn default() -> Self {
        Self {
            prediction: "unknown".to_string(),
            confidence: 0.0f64,
        }
    }
}

pub struct MLClient {
    base_url: String,
    api_key: String,
    client: Client,
}

impl MLClient {
    pub fn new(base_url: String, api_key: String) -> Self {
        Self {
            base_url,
            api_key,
            client: Client::new(),
        }
    }

    pub async fn classify_workout(
        &self,
        heart_rate_data: &[crate::models::workout_data::HeartRateData],
        user_resting_hr: i32,
        user_max_hr: i32,
        activity_type: Option<String>,
    ) -> Result<ClassifyResponse, Box<dyn std::error::Error>> {
        // Convert heart rate data to the format expected by ML service
        let hr_samples: Vec<HeartRateSample> = heart_rate_data
            .iter()
            .map(|sample| HeartRateSample {
                timestamp: sample.timestamp.to_rfc3339(),
                heart_rate: sample.heart_rate,
            })
            .collect();

        let request = ClassifyRequest {
            heart_rate_data: hr_samples,
            user_resting_hr,
            user_max_hr,
            activity_type,
        };

        let url = format!("{}/classify", self.base_url);

        tracing::debug!("🤖 Calling ML service at {}", url);

        let response = self
            .client
            .post(&url)
            .header("X-API-Key", &self.api_key)
            .json(&request)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            tracing::error!("❌ ML service returned error {}: {}", status, error_text);
            return Err(format!("ML service error: {} - {}", status, error_text).into());
        }

        let classification = response.json::<ClassifyResponse>().await?;

        tracing::info!(
            "✅ ML classification: {} (confidence: {:.2}%)",
            classification.prediction,
            classification.confidence * 100.0
        );

        Ok(classification)
    }
}
