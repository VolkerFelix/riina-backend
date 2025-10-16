use actix_web::web;
use crate::handlers::feed::newsfeed;

pub fn init_feed_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(newsfeed::get_unified_feed));
}