use actix_web::{web, App, HttpServer};
use actix_web::dev::Server;
use tracing_actix_web::TracingLogger;
use sqlx::PgPool;
use std::net::TcpListener;
pub mod config;
mod routes;
mod handlers;
mod models;
mod utils;
pub mod telemetry;
mod middleware;
mod db;
use crate::routes::init_routes;
use crate::config::jwt::JwtSettings;

pub fn run(
    listener: TcpListener,
    db_pool: PgPool,
    jwt_settings: JwtSettings,
    redis_client: redis::Client
) -> Result<Server, std::io::Error> {
    // Wrap using web::Data, which boils down to an Arc smart pointer
    let db_pool = web::Data::new(db_pool);
    let jwt_settings = web::Data::new(jwt_settings);

    let server = HttpServer::new( move || {
        App::new()
            .wrap(TracingLogger::default())
            .configure(init_routes)
            // Get a pointer copy and attach it to the application state
            .app_data(db_pool.clone())
            .app_data(jwt_settings.clone())
            .app_data(redis_client.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}