use actix_web::web;

use crate::handlers::notification_handler;

pub fn init_notification_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/register")
            .route(web::post().to(notification_handler::register_push_token))
    )
    .service(
        web::resource("/unregister")
            .route(web::post().to(notification_handler::unregister_push_token))
    )
    .service(
        web::resource("/tokens")
            .route(web::get().to(notification_handler::get_user_tokens))
    )
    .service(
        web::resource("/send")
            .route(web::post().to(notification_handler::send_notification))
    );
}
