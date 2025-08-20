use serde::{Deserialize, Serialize};
use aws_sdk_s3::{config::Builder as S3ConfigBuilder, Client as S3Client};
use aws_config::Region;
use aws_sdk_s3::config::{Credentials, SharedCredentialsProvider};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MinIOSettings {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket_name: String,
    pub region: String,
}

impl MinIOSettings {
    pub fn new() -> Result<Self, config::ConfigError> {
        let settings = config::Config::builder()
            .add_source(config::Environment::with_prefix("MINIO").separator("__"))
            .build()?;

        settings.try_deserialize()
    }

    pub async fn create_s3_client(&self) -> Result<S3Client, Box<dyn std::error::Error + Send + Sync>> {
        let creds = Credentials::new(
            &self.access_key,
            &self.secret_key,
            None, // No session token
            None, // No expiration
            "custom-minio", // Provider name
        );

        let config = S3ConfigBuilder::new()
            .endpoint_url(&self.endpoint)
            .credentials_provider(SharedCredentialsProvider::new(creds))
            .region(Region::new(self.region.clone()))
            .force_path_style(true) // Important for MinIO
            .behavior_version_latest() // Required by AWS SDK v1.102+
            .build();

        Ok(S3Client::from_conf(config))
    }
}

impl Default for MinIOSettings {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:9000".to_string(),
            access_key: "minioadmin".to_string(),
            secret_key: "minioadmin123".to_string(),
            bucket_name: "evolveme-workout-media".to_string(),
            region: "us-east-1".to_string(),
        }
    }
}