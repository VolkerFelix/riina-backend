use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage, web,
    error::ErrorUnauthorized,
};
use futures_util::future::LocalBoxFuture;
use std::{
    future::{ready, Ready},
    rc::Rc,
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use secrecy::ExposeSecret;

use crate::config::jwt::JwtSettings;
use crate::middleware::auth::Claims;
use crate::models::user::{UserRole, UserStatus};

pub struct AdminMiddleware;

impl<S, B> Transform<S, ServiceRequest> for AdminMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AdminMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AdminMiddlewareService {
            service: Rc::new(service),
        }))
    }
}

pub struct AdminMiddlewareService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for AdminMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();

        Box::pin(async move {
            // Extract JWT from Authorization header
            let auth_header = req.headers().get("authorization");
            let jwt_settings = req.app_data::<web::Data<JwtSettings>>().cloned();

            // No JWT settings in app state
            if jwt_settings.is_none() {
                return Err(ErrorUnauthorized("JWT settings not found"));
            }

            // No auth header
            if auth_header.is_none() {
                return Err(ErrorUnauthorized("No authorization header"));
            }

            let auth_header = auth_header.unwrap().to_str().unwrap_or_default();
            
            // Check if it's a Bearer token
            if !auth_header.starts_with("Bearer ") {
                return Err(ErrorUnauthorized("Invalid authorization header format"));
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
                    tracing::error!("Failed to decode admin token: {:?}", e);
                    return Err(ErrorUnauthorized("Invalid token"));
                }
            };

            // Check if user has admin privileges
            let claims = &token_data.claims;
            
            // Only allow active users with admin or superadmin roles
            match claims.status {
                UserStatus::Active => {},
                _ => {
                    tracing::warn!("Inactive user attempted admin access: {}", claims.username);
                    return Err(ErrorUnauthorized("Account is not active"));
                }
            }
            
            match claims.role {
                UserRole::Admin | UserRole::SuperAdmin => {},
                _ => {
                    tracing::warn!("Non-admin user attempted admin access: {} (role: {:?})", claims.username, claims.role);
                    return Err(ErrorUnauthorized("Insufficient privileges"));
                }
            }
            
            // Store the claims in the request extensions for handlers to access
            req.extensions_mut().insert(token_data.claims);

            let res = service.call(req).await?;
            Ok(res)
        })
    }
}