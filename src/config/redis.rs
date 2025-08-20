use serde::Deserialize;
use secrecy::{ExposeSecret, SecretString};

#[derive(Debug, Deserialize)]
pub struct RedisSettings {
    pub host: String,
    pub port: u16,
    pub password: SecretString
}

impl RedisSettings {
    pub fn get_redis_url(&self) -> SecretString {
        SecretString::new(format!("redis://:{}@{}:{}", self.password.expose_secret(), self.host, self.port).into_boxed_str())
    }
}