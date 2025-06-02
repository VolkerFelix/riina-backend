use serde::Deserialize;

// Query parameter struct for token
#[derive(Deserialize)]
pub struct TokenQuery {
    pub token: String,
} 