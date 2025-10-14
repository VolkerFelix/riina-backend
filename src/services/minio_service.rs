use bytes::Bytes;
use std::sync::Arc;
use uuid::Uuid;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3::error::SdkError;
use aws_sdk_s3::operation::create_bucket::CreateBucketError;

use crate::config::minio::MinIOSettings;

#[derive(Clone, Debug)]
pub struct MinIOService {
    pub internal_client: Arc<S3Client>,
    pub external_client: Option<Arc<S3Client>>,
    bucket_name: String,
    settings: MinIOSettings,
}

impl MinIOService {
    pub async fn new(settings: &MinIOSettings) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bucket_name = settings.bucket_name.clone();
        let internal_client = settings.create_internal_s3_client().await;
        let internal_client = Arc::new(internal_client);
        let external_client = match settings.create_external_s3_client().await {
            Some(external_client) => {
                tracing::info!("✅ MinIO external client: {:?}", external_client);
                Some(Arc::new(external_client))
            }
            None => {
            tracing::info!("❌ MinIO external client not created");
            None
        }};

        let service = Self {    
            internal_client,
            external_client,
            bucket_name,
            settings: settings.clone(),
        };
        
        // Initialize bucket
        service.init_bucket().await?;
        
        Ok(service)
    }
    

    async fn init_bucket(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("🗄️ Initializing MinIO bucket: {}", self.bucket_name);
        
        // Check if bucket exists
        let bucket_exists = match self.internal_client
            .head_bucket()
            .bucket(&self.bucket_name)
            .send()
            .await 
        {
            Ok(_) => true,
            Err(_) => false,
        };

        if !bucket_exists {
            tracing::info!("📦 Creating MinIO bucket: {}", self.bucket_name);
            match self.internal_client
                .create_bucket()
                .bucket(&self.bucket_name)
                .send()
                .await
            {
                Ok(_) => {
                    tracing::info!("✅ MinIO bucket created successfully");
                }
                Err(e) => {
                    // Handle specific AWS S3/MinIO error types
                    match &e {
                        SdkError::ServiceError(service_err) => {
                            match service_err.err() {
                                CreateBucketError::BucketAlreadyOwnedByYou(_) => {
                                    tracing::info!("✅ MinIO bucket already exists (owned by us)");
                                    // This is fine - bucket exists and we own it
                                }
                                CreateBucketError::BucketAlreadyExists(_) => {
                                    // This is only expected during local development and testing
                                    if self.settings.testing {
                                        tracing::info!("✅ MinIO bucket already exists (owned by us) - testing mode");
                                    } else {
                                        tracing::error!("❌ Failed to create MinIO bucket: {:?}", service_err.err());
                                        return Err(Box::new(e));
                                    }
                                }
                                _ => {
                                    tracing::error!("❌ Failed to create MinIO bucket: {:?}", service_err.err());
                                    return Err(Box::new(e));
                                }
                            }
                        }
                        _ => {
                            tracing::error!("❌ Failed to create MinIO bucket (SDK error): {}", e);
                            return Err(Box::new(e));
                        }
                    }
                }
            }
        } else {
            tracing::info!("✅ MinIO bucket already exists");
        }

        Ok(())
    }

    pub async fn upload_file(
        &self,
        file_data: Bytes,
        file_name: &str,
        content_type: &str,
        user_id: Uuid,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Create a unique path with user_id prefix for organization
        let object_key = format!("users/{}/{}", user_id, file_name);
        
        tracing::info!("📤 Uploading file to MinIO: {} (size: {} bytes)", object_key, file_data.len());

        let byte_stream = ByteStream::from(file_data);

        match self.internal_client
            .put_object()
            .bucket(&self.bucket_name)
            .key(&object_key)
            .body(byte_stream)
            .content_type(content_type)
            .metadata("user_id", user_id.to_string())
            .metadata("uploaded_at", chrono::Utc::now().to_rfc3339())
            .send()
            .await
        {
            Ok(_) => {
                tracing::info!("✅ File uploaded successfully to MinIO: {}", object_key);
                Ok(object_key)
            }
            Err(e) => {
                tracing::error!("❌ Failed to upload file to MinIO: {}", e);
                Err(Box::new(e))
            }
        }
    }

    pub async fn get_file(
        &self,
        object_key: &str,
    ) -> Result<(Bytes, String), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("📥 Downloading file from MinIO: {}", object_key);

        match self.internal_client
            .get_object()
            .bucket(&self.bucket_name)
            .key(object_key)
            .send()
            .await
        {
            Ok(response) => {
                // Get content type from metadata
                let content_type = response
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();

                // Read all bytes from the response
                let bytes_data = response.body.collect().await?;
                let bytes = bytes_data.into_bytes();

                tracing::info!("✅ File downloaded successfully from MinIO: {} (size: {} bytes)", 
                      object_key, bytes.len());
                
                Ok((bytes, content_type))
            }
            Err(e) => {
                tracing::warn!("❌ File not found in MinIO: {} - {}", object_key, e);
                Err(Box::new(e))
            }
        }
    }

    pub async fn delete_file(
        &self,
        object_key: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        tracing::info!("🗑️ Deleting file from MinIO: {}", object_key);

        match self.internal_client
            .delete_object()
            .bucket(&self.bucket_name)
            .key(object_key)
            .send()
            .await
        {
            Ok(_) => {
                tracing::info!("✅ File deleted successfully from MinIO: {}", object_key);
                Ok(())
            }
            Err(e) => {
                tracing::error!("❌ Failed to delete file from MinIO: {}", e);
                Err(Box::new(e))
            }
        }
    }

    pub fn generate_file_url(&self, object_key: &str) -> String {
        // Handle different object key patterns
        if let Some(path_without_users) = object_key.strip_prefix("users/") {
            // Workout media files: users/{user_id}/{filename} -> /health/workout-media/{user_id}/{filename}
            format!("/health/workout-media/{}", path_without_users)
        } else if let Some(path_without_profile) = object_key.strip_prefix("profile-pictures/") {
            // Profile pictures: profile-pictures/{user_id}/{filename} -> /profile/picture/{user_id}/{filename}
            format!("/profile/picture/{}", path_without_profile)
        } else {
            // Fallback for any non-standard object keys
            format!("/health/workout-media/{}", object_key)
        }
    }

    pub async fn generate_presigned_download_url(&self, object_key: &str, expires_in_seconds: u32) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        use aws_sdk_s3::presigning::PresigningConfig;
        use std::time::Duration;
        
        tracing::debug!("🔗 Generating presigned download URL for object: {}", object_key);
        
        let presigning_config = PresigningConfig::builder()
            .expires_in(Duration::from_secs(expires_in_seconds as u64))
            .build()?;
            
        let client = self.external_client.as_ref().unwrap_or(&self.internal_client);
        let presigned_req = client
            .get_object()
            .bucket(&self.bucket_name)
            .key(object_key)
            .presigned(presigning_config)
            .await?;
            
        let signed_url = presigned_req.uri().to_string();
        
        tracing::debug!("✅ Generated presigned download URL (expires in {}s): {}", expires_in_seconds, signed_url);
        
        Ok(signed_url)
    }

    pub async fn generate_presigned_upload_url(&self, object_key: &str, content_type: &str, expected_hash: &str, expires_in_seconds: u32) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        use aws_sdk_s3::presigning::PresigningConfig;
        use std::time::Duration;
        use base64::{Engine as _, engine::general_purpose};
        
        tracing::debug!("📤 Generating presigned upload URL for object: {} with hash: {}", object_key, expected_hash);
        
        // Convert hex hash to base64 for S3 checksum
        let hash_bytes = hex::decode(expected_hash)
            .map_err(|e| format!("Invalid hash format: {}", e))?;
        let checksum_sha256 = general_purpose::STANDARD.encode(&hash_bytes);
        
        let presigning_config = PresigningConfig::builder()
            .expires_in(Duration::from_secs(expires_in_seconds as u64))
            .build()?;
            
        let client = self.external_client.as_ref().unwrap_or(&self.internal_client);
        let presigned_req = client
            .put_object()
            .bucket(&self.bucket_name)
            .key(object_key)
            .content_type(content_type)
            .checksum_sha256(&checksum_sha256)  // MinIO will verify this matches the uploaded file
            .presigned(presigning_config)
            .await?;
            
        let signed_url = presigned_req.uri().to_string();
        
        tracing::debug!("✅ Generated presigned upload URL with checksum verification (expires in {}s): {}", expires_in_seconds, signed_url);
        
        Ok(signed_url)
    }
}