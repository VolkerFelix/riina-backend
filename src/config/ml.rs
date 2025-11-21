use serde::Deserialize;
use secrecy::SecretString;

#[derive(Deserialize, Debug, Clone)]
pub struct MLSettings {
    pub service_url: String,
    pub api_key: SecretString,
}

impl MLSettings {
    pub fn new(service_url: String, api_key: SecretString) -> Self {
        Self {
            service_url,
            api_key,
        }
    }
}
