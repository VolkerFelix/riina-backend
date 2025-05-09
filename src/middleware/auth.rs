// src/middleware/auth.rs
use std::{future::{ready, Ready}};
use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform}, error::ErrorUnauthorized, http::header, web, Error, HttpMessage
};
use futures_util::future::LocalBoxFuture;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde::{Deserialize, Serialize};
use secrecy::ExposeSecret;

use crate::config::jwt::JwtSettings;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,  // Subject (user id)
    pub username: String,
    pub exp: usize,   // Expiration time (as UTC timestamp)
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
        // Extract JWT from Authorization header
        let auth_header = req.headers().get(header::AUTHORIZATION);
        let jwt_settings = req.app_data::<web::Data<JwtSettings>>().cloned();

        // No JWT settings in app state
        if jwt_settings.is_none() {
            return Box::pin(async move {
                Err(ErrorUnauthorized("JWT settings not found"))
            });
        }

        // No auth header
        if auth_header.is_none() {
            return Box::pin(async move {
                Err(ErrorUnauthorized("No authorization header"))
            });
        }

        let auth_header = auth_header.unwrap().to_str().unwrap_or_default();
        
        // Check if it's a Bearer token
        if !auth_header.starts_with("Bearer ") {
            return Box::pin(async move {
                Err(ErrorUnauthorized("Invalid authorization header format"))
            });
        }

        // Extract the token
        let token = &auth_header[7..]; // Skip "Bearer "
        let jwt_settings = jwt_settings.unwrap();

        // Decode the token
        let token_data = match decode::<Claims>(
            token,
            &DecodingKey::from_secret(jwt_settings.secret.expose_secret().as_bytes()),
            &Validation::new(Algorithm::HS256),
        ) {
            Ok(c) => c,
            Err(e) => {
                return Box::pin(async move {
                    tracing::error!("Failed to decode token: {:?}", e);
                    Err(ErrorUnauthorized("Invalid token"))
                });
            }
        };

        // Store the claims in the request extensions for handlers to access
        req.extensions_mut().insert(token_data.claims);

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = fut.await?;
            Ok(res)
        })
    }
}