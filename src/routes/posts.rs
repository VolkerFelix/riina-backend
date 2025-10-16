use actix_web::web;
use crate::handlers::posts::post_handler::{
    create_post, update_post, delete_post, get_post
};

pub fn init_posts_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/")
            .route(web::post().to(create_post))
    );

    cfg.service(
        web::resource("/{post_id}")
            .route(web::get().to(get_post))
            .route(web::patch().to(update_post))
            .route(web::delete().to(delete_post))
    );
}
