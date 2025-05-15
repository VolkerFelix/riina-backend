use serde::Deserialize;
use secrecy::SecretString;

#[derive(Debug, Deserialize)]
pub struct RedisSettings {
    pub host: String,
    pub port: u16,
    pub password: SecretString
}

