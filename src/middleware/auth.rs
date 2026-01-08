// src/middleware/auth.rs
use std::{future::{ready, Ready}};
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform}, error::ErrorUnauthorized, http::header, web, Error, HttpMessage
};
use futures_util::future::LocalBoxFuture;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use secrecy::ExposeSecret;

use uuid::Uuid;

use crate::config::jwt::JwtSettings;
use crate::models::user::{UserRole, UserStatus};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,  // Subject (user id)
    pub username: String,
    pub role: UserRole,
    pub status: UserStatus,
    pub exp: usize,   // Expiration time (as UTC timestamp)
}

impl Claims {
    /// Parse the user ID from the claims subject field.
    /// Returns None if the UUID is invalid.
    pub fn user_id(&self) -> Option<Uuid> {
        Uuid::parse_str(&self.sub).ok()
    }
}

/// Shared JWT validation logic used by both auth and admin middlewares.
/// Extracts and validates a JWT token from the request, returning the decoded claims.
pub fn validate_jwt_from_request(req: &ServiceRequest) -> Result<Claims, Error> {
    // Get JWT settings from app state
    let jwt_settings = req.app_data::<web::Data<JwtSettings>>()
        .ok_or_else(|| ErrorUnauthorized("JWT settings not found"))?;

    // Extract Authorization header
    let auth_header = req.headers()
        .get(header::AUTHORIZATION)
        .ok_or_else(|| ErrorUnauthorized("No authorization header"))?
        .to_str()
        .map_err(|_| ErrorUnauthorized("Invalid authorization header"))?;

    // Check Bearer token format
    if !auth_header.starts_with("Bearer ") {
        return Err(ErrorUnauthorized("Invalid authorization header format"));
    }

    // Extract and decode the token
    let token = &auth_header[7..]; // Skip "Bearer "
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_settings.secret.expose_secret().as_bytes()),
        &Validation::new(Algorithm::HS256),
    ).map_err(|e| {
        tracing::error!("Failed to decode token: {:?}", e);
        ErrorUnauthorized("Invalid token")
    })?;

    Ok(token_data.claims)
}

// Create the middleware
pub struct AuthMiddleware;

// Middleware factory
impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = AuthMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService { service }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Validate JWT and extract claims using shared function
        let claims = match validate_jwt_from_request(&req) {
            Ok(claims) => claims,
            Err(e) => return Box::pin(async move { Err(e) }),
        };

        // Store the claims in the request extensions for handlers to access
        req.extensions_mut().insert(claims);

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            Ok(res)
        })
    }
}