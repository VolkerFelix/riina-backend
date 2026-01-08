use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
    error::{ErrorUnauthorized, ErrorForbidden},
};
use futures_util::future::LocalBoxFuture;
use std::{
    future::{ready, Ready},
    rc::Rc,
};

use crate::middleware::auth::validate_jwt_from_request;
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

        // Validate JWT and extract claims using shared function
        let claims = match validate_jwt_from_request(&req) {
            Ok(claims) => claims,
            Err(e) => return Box::pin(async move { Err(e) }),
        };

        // Check if user has admin privileges
        // Only allow active users with admin or superadmin roles
        match claims.status {
            UserStatus::Active => {},
            _ => {
                tracing::warn!("Inactive user attempted admin access: {}", claims.username);
                return Box::pin(async move { Err(ErrorUnauthorized("Account is not active")) });
            }
        }

        match claims.role {
            UserRole::Admin | UserRole::SuperAdmin => {},
            _ => {
                tracing::warn!("Non-admin user attempted admin access: {} (role: {:?})", claims.username, claims.role);
                return Box::pin(async move { Err(ErrorForbidden("Insufficient privileges")) });
            }
        }

        // Store the claims in the request extensions for handlers to access
        req.extensions_mut().insert(claims);

        Box::pin(async move {
            let res = service.call(req).await?;
            Ok(res)
        })
    }
}