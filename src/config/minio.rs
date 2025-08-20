use serde::Deserialize;
use aws_sdk_s3::{config::Builder as S3ConfigBuilder, Client as S3Client};
use aws_config::Region;
use aws_sdk_s3::config::{Credentials, SharedCredentialsProvider};
use secrecy::{ExposeSecret, SecretString};

#[derive(Debug, Deserialize, Clone)]
pub struct MinIOSettings {
    pub endpoint: String,
    pub access_key: SecretString,
    pub secret_key: SecretString,
    pub bucket_name: String,
    pub region: String,
    pub testing: bool,
}

impl MinIOSettings {
    pub async fn create_s3_client(&self) -> Result<S3Client, Box<dyn std::error::Error + Send + Sync>> {
        let creds = Credentials::new(
            self.access_key.expose_secret(),
            self.secret_key.expose_secret(),
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