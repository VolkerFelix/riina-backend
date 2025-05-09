use serde::Deserialize;


#[derive(Debug, Deserialize)]
pub struct RedisSettings {
    pub host: String,
    pub port: u16,
}

