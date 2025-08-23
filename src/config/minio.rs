use serde::Deserialize;
use aws_sdk_s3::{config::Builder as S3ConfigBuilder, Client as S3Client};
use aws_config::Region;
use aws_sdk_s3::config::{Credentials, SharedCredentialsProvider};
use secrecy::{ExposeSecret, SecretString};

#[derive(Debug, Deserialize, Clone)]
pub struct MinIOSettings {
    pub endpoint: String, // Internal endpoint for service-to-service communication
    #[serde(default)]
    pub external_endpoint: Option<String>, // Browser-accessible endpoint for presigned URLs
    pub access_key: SecretString,
    pub secret_key: SecretString,
    pub bucket_name: String,
    pub region: String,
    pub testing: bool,
}

impl MinIOSettings {
    /// Get the endpoint that should be used for presigned URLs (accessible by browsers)
    pub fn get_presigned_url_endpoint(&self) -> &str {
        self.external_endpoint.as_ref().unwrap_or(&self.endpoint)
    }
}

impl MinIOSettings {
    pub async fn create_internal_s3_client(&self) -> S3Client {
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

        S3Client::from_conf(config)
    }

    pub async fn create_external_s3_client(&self) -> Option<S3Client> {
        if let Some(external_endpoint) = &self.external_endpoint {
            let creds = Credentials::new(
                self.access_key.expose_secret(),
                self.secret_key.expose_secret(),
                None, // No session token
                None, // No expiration
                "custom-minio", // Provider name
            );
            let config = S3ConfigBuilder::new()
                .endpoint_url(external_endpoint.clone())
                .credentials_provider(SharedCredentialsProvider::new(creds))
                .region(Region::new(self.region.clone()))
                .force_path_style(true) // Important for MinIO
                .behavior_version_latest() // Required by AWS SDK v1.102+
                .build();

            Some(S3Client::from_conf(config))
        } else {
            None
        }
    }
}