use actix_web::web;

pub mod registration;
pub mod backend_health;
pub mod auth;
pub mod protected;
pub mod health_data;

use crate::middleware::auth::AuthMiddleware;

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(registration::register)
        .service(backend_health::backend_health)
        .service(auth::login);

    cfg.service(
        web::scope("/protected")
            .service(protected::protected_resource)
    );

    cfg.service(
        web::scope("/health")
            .wrap(AuthMiddleware)
            .service(health_data::upload_health)
    );
}