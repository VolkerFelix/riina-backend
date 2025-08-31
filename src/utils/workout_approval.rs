use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkoutApprovalToken {
    pub user_id: Uuid,
    pub workout_id: String,
    pub workout_start: DateTime<Utc>,
    pub workout_end: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl WorkoutApprovalToken {
    pub fn new(
        user_id: Uuid,
        workout_id: String,
        workout_start: DateTime<Utc>,
        workout_end: DateTime<Utc>,
        validity_minutes: i64,
    ) -> Self {
        Self {
            user_id,
            workout_id,
            workout_start,
            workout_end,
            expires_at: Utc::now() + Duration::minutes(validity_minutes),
        }
    }

    pub fn generate_token(&self, secret: &SecretString) -> Result<String, String> {
        // Create the payload to sign
        let payload = format!(
            "{}|{}|{}|{}|{}",
            self.user_id,
            self.workout_id,
            self.workout_start.timestamp(),
            self.workout_end.timestamp(),
            self.expires_at.timestamp()
        );

        // Create HMAC
        let mut mac = HmacSha256::new_from_slice(secret.expose_secret().as_bytes())
            .map_err(|e| format!("Failed to create HMAC: {}", e))?;
        
        mac.update(payload.as_bytes());
        let result = mac.finalize();
        let signature = hex::encode(result.into_bytes());

        // Combine payload and signature
        Ok(format!("{}|{}", payload, signature))
    }

    pub fn validate_token(
        token: &str,
        secret: &SecretString,
        expected_user_id: Uuid,
    ) -> Result<Self, String> {
        // Split token into payload and signature
        let parts: Vec<&str> = token.split('|').collect();
        if parts.len() != 6 {
            return Err("Invalid token format".to_string());
        }

        // Extract components
        let user_id = Uuid::parse_str(parts[0])
            .map_err(|_| "Invalid user ID in token".to_string())?;
        let workout_id = parts[1].to_string();
        let workout_start = DateTime::from_timestamp(
            parts[2].parse::<i64>().map_err(|_| "Invalid start timestamp")?,
            0
        ).ok_or("Invalid start timestamp")?;
        let workout_end = DateTime::from_timestamp(
            parts[3].parse::<i64>().map_err(|_| "Invalid end timestamp")?,
            0
        ).ok_or("Invalid end timestamp")?;
        let expires_at = DateTime::from_timestamp(
            parts[4].parse::<i64>().map_err(|_| "Invalid expiry timestamp")?,
            0
        ).ok_or("Invalid expiry timestamp")?;
        let provided_signature = parts[5];

        // Verify user ID matches
        if user_id != expected_user_id {
            return Err("Token user ID does not match".to_string());
        }

        // Check expiration
        if expires_at < Utc::now() {
            return Err("Token has expired".to_string());
        }

        // Reconstruct payload
        let payload = format!(
            "{}|{}|{}|{}|{}",
            user_id,
            workout_id,
            workout_start.timestamp(),
            workout_end.timestamp(),
            expires_at.timestamp()
        );

        // Verify signature
        let mut mac = HmacSha256::new_from_slice(secret.expose_secret().as_bytes())
            .map_err(|e| format!("Failed to create HMAC: {}", e))?;
        
        mac.update(payload.as_bytes());
        let result = mac.finalize();
        let expected_signature = hex::encode(result.into_bytes());

        if provided_signature != expected_signature {
            return Err("Invalid token signature".to_string());
        }

        Ok(Self {
            user_id,
            workout_id,
            workout_start,
            workout_end,
            expires_at,
        })
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at < Utc::now()
    }
}