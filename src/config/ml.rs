use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct MLSettings {
    pub service_url: String,
}

impl MLSettings {
    pub fn new(service_url: String) -> Self {
        Self {
            service_url,
        }
    }
}
