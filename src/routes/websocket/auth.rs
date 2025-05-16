use actix_web::web;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use secrecy::ExposeSecret;
use crate::config::jwt::JwtSettings;
use crate::middleware::auth::Claims;

// Helper function to decode JWT token
pub fn decode_token(token: &str, jwt_settings: &web::Data<JwtSettings>) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_settings.secret.expose_secret().as_bytes()),
        &Validation::new(Algorithm::HS256)
    ).map(|data| data.claims)
} 