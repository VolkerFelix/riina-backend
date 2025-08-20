use crate::config::minio::MinIOSettings;
use bytes::Bytes;
use aws_sdk_s3::{Client as S3Client, primitives::ByteStream};
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct MinIOService {
    client: Arc<S3Client>,
    bucket_name: String,
}

impl MinIOService {
    pub async fn new(settings: MinIOSettings) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let bucket_name = settings.bucket_name.clone();
        let client = Arc::new(settings.create_s3_client().await?);
        
        let service = Self {
            client,
            bucket_name,
        };
        
        // Initialize bucket
        service.init_bucket().await?;
        
        Ok(service)
    }

    async fn init_bucket(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("🗄️ Initializing MinIO bucket: {}", self.bucket_name);
        
        // Check if bucket exists
        let bucket_exists = match self.client
            .head_bucket()
            .bucket(&self.bucket_name)
            .send()
            .await 
        {
            Ok(_) => true,
            Err(_) => false,
        };

        if !bucket_exists {
            info!("📦 Creating MinIO bucket: {}", self.bucket_name);
            self.client
                .create_bucket()
                .bucket(&self.bucket_name)
                .send()
                .await?;
            info!("✅ MinIO bucket created successfully");
        } else {
            info!("✅ MinIO bucket already exists");
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
        
        info!("📤 Uploading file to MinIO: {} (size: {} bytes)", object_key, file_data.len());

        let byte_stream = ByteStream::from(file_data);

        match self.client
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
                info!("✅ File uploaded successfully to MinIO: {}", object_key);
                Ok(object_key)
            }
            Err(e) => {
                error!("❌ Failed to upload file to MinIO: {}", e);
                Err(Box::new(e))
            }
        }
    }

    pub async fn get_file(
        &self,
        object_key: &str,
    ) -> Result<(Bytes, String), Box<dyn std::error::Error + Send + Sync>> {
        info!("📥 Downloading file from MinIO: {}", object_key);

        match self.client
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

                info!("✅ File downloaded successfully from MinIO: {} (size: {} bytes)", 
                      object_key, bytes.len());
                
                Ok((bytes, content_type))
            }
            Err(e) => {
                warn!("❌ File not found in MinIO: {} - {}", object_key, e);
                Err(Box::new(e))
            }
        }
    }

    pub async fn delete_file(
        &self,
        object_key: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("🗑️ Deleting file from MinIO: {}", object_key);

        match self.client
            .delete_object()
            .bucket(&self.bucket_name)
            .key(object_key)
            .send()
            .await
        {
            Ok(_) => {
                info!("✅ File deleted successfully from MinIO: {}", object_key);
                Ok(())
            }
            Err(e) => {
                error!("❌ Failed to delete file from MinIO: {}", e);
                Err(Box::new(e))
            }
        }
    }

    pub fn generate_file_url(&self, object_key: &str) -> String {
        // For local development, return a URL that points to our serving endpoint
        // In production, you might want to use presigned URLs or a CDN
        format!("/api/workout-media/{}", object_key.replace("users/", "").replace("/", "_"))
    }
}