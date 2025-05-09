// src/models/auth.rs
use serde::{Deserialize, Serialize};
use secrecy::SecretString;

#[derive(Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    #[serde(serialize_with = "crate::models::user::serialize_secret_string", 
            deserialize_with = "crate::models::user::deserialize_secret_string")]
    pub password: SecretString,
}

#[derive(Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
}