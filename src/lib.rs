use actix_web::{http,web, App, HttpServer};
use actix_web::dev::Server;
use tracing_actix_web::TracingLogger;
use sqlx::PgPool;
use std::net::TcpListener;
use actix_cors::Cors;

pub mod config;
mod routes;
mod handlers;
pub mod models;
pub mod utils;
mod middleware;
pub mod db;
pub mod game;
pub mod league;
pub mod workout;
pub mod services;
use crate::routes::init_routes;
use crate::config::jwt::JwtSettings;
use crate::services::{SchedulerService, MinIOService};
use std::sync::Arc;

pub fn run(
    listener: TcpListener,
    db_pool: PgPool,
    jwt_settings: JwtSettings,
    redis_client: Arc<redis::Client>,
    scheduler_service: Arc<SchedulerService>,
    minio_service: MinIOService
) -> Result<Server, std::io::Error> {
    // Wrap using web::Data, which boils down to an Arc smart pointer
    let db_pool_data = web::Data::new(db_pool.clone());
    let jwt_settings = web::Data::new(jwt_settings);
    let scheduler_service = web::Data::new(scheduler_service);
    let redis_client_data = web::Data::new(redis_client.clone());
    
    
    // Wrap MinIOService
    let minio_service_data = web::Data::new(minio_service);


    let server = HttpServer::new( move || {
        let cors = Cors::default()
            .allowed_origin("http://localhost:3000")
            .allowed_origin("http://localhost:3001")
            .allowed_origin("https://riina.fly.dev")
            .allowed_origin("https://evolveme-admin.fly.dev")
            .allowed_origin("https://riina-dev.fly.dev")
            .allowed_origin("https://evolveme-admin-dev.fly.dev")
            .allowed_methods(vec!["GET", "POST", "PUT", "DELETE", "PATCH"])
            .allowed_headers(vec![
                http::header::AUTHORIZATION,
                http::header::ACCEPT,
                http::header::CONTENT_TYPE,
                http::header::UPGRADE,
                http::header::CONNECTION,
            ])
            .supports_credentials()
            .max_age(3600);

        let mut app = App::new()
            .wrap(TracingLogger::default())
            .wrap(cors)
            // Get a pointer copy and attach it to the application state
            .app_data(db_pool_data.clone())
            .app_data(jwt_settings.clone())
            .app_data(scheduler_service.clone())
            .app_data(minio_service_data.clone());
        app = app.app_data(redis_client_data.clone());

        app.configure(init_routes)
    })
    .listen(listener)?
    .run();

    Ok(server)
}