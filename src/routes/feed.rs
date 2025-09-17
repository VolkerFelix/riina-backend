use actix_web::web;
use crate::handlers::feed;

pub fn init_feed_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(feed::get_newsfeed));
}