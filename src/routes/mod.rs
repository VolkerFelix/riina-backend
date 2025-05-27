use actix_web::web;

pub mod registration;
pub mod backend_health;
pub mod auth;
pub mod protected;
pub mod health_data;
pub mod websocket;
pub mod llm;

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
            .service(health_data::get_state)
    );
    cfg.service(
        web::scope("/llm")
            .wrap(AuthMiddleware)
            .service(llm::generate_twin_thought)
            .service(llm::handle_user_response)
            .service(llm::get_twin_history)
            .service(llm::update_user_reaction)
            .service(llm::trigger_health_reaction)
    );
    cfg.service(
        web::resource("/ws")
            .route(web::get().to(websocket::ws_route))
    );
}