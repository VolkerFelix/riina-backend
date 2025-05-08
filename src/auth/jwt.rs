use actix_web::{
    dev::Payload, error::ErrorUnauthorized, web, Error as ActixError, FromRequest, HttpRequest,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::env;
use std::future::{ready, Ready};
use uuid::Uuid;

use crate::models::user::TokenClaims;

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtToken {
    pub token: String,
}

pub struct AuthenticatedUser {
    pub user_id: Uuid,
}

impl FromRequest for AuthenticatedUser {
    type Error = ActixError;
    type Future = Ready<Result<Self, Self::Error>>;

    fn from_request(req: &HttpRequest, _: &mut Payload) -> Self::Future {
        // Extract the token from the Authorization header
        let auth_header = req.headers().get("Authorization");
        if auth_header.is_none() {
            return ready(Err(ErrorUnauthorized("No authorization header")));
        }

        let auth_value = auth_header.unwrap().to_str();
        if auth_value.is_err() {
            return ready(Err(ErrorUnauthorized("Invalid authorization header")));
        }

        let auth_value = auth_value.unwrap();
        if !auth_value.starts_with("Bearer ") {
            return ready(Err(ErrorUnauthorized("Invalid authorization scheme")));
        }

        let token = auth_value[7..].trim();
        if token.is_empty() {
            return ready(Err(ErrorUnauthorized("Empty token")));
        }

        // Decode and validate the token
        let result = decode_token(token);
        match result {
            Ok(user_id) => ready(Ok(AuthenticatedUser { user_id })),
            Err(e) => ready(Err(ErrorUnauthorized(e.to_string()))),
        }
    }
}

pub fn generate_token(user_id: Uuid) -> Result<String, jsonwebtoken::errors::Error> {
    let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    
    let expiration_hours = env::var("JWT_EXPIRATION_HOURS")
        .unwrap_or_else(|_| "24".to_string())
        .parse::<i64>()
        .unwrap_or(24);
    
    let now = Utc::now();
    let expires_at = now + Duration::hours(expiration_hours);
    
    let claims = TokenClaims {
        sub: user_id.to_string(),
        exp: expires_at.timestamp() as usize,
        iat: now.timestamp() as usize,
    };
    
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )
}

pub fn decode_token(token: &str) -> Result<Uuid, jsonwebtoken::errors::Error> {
    let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
    
    let token_data = decode::<TokenClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    )?;
    
    let user_id = Uuid::parse_str(&token_data.claims.sub)
        .map_err(|_| jsonwebtoken::errors::ErrorKind::InvalidSubject)?;
    
    Ok(user_id)
}