use std::sync::Arc;
use secrecy::ExposeSecret;
use redis::Client;

use crate::config::redis::RedisSettings;

#[derive(Clone, Debug)]
pub struct RedisService {
    pub client: Arc<Client>,
}

impl RedisService {
    pub async fn new(settings: &RedisSettings) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = match Client::open(settings.get_redis_url().expose_secret()) {
                Ok(client) => {
                    tracing::info!("Redis client created successfully");
                    client
                },
                Err(e) => {
                    tracing::error!("Failed to create Redis client: {}", e);
                    return Err(Box::new(e));
                }
            };
        let client = Arc::new(client);
        Ok(Self { client })
    }
}