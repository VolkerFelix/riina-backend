use actix_cors::Cors;
use actix_web::{middleware, App, HttpServer};
use dotenv::dotenv;
use log::info;
use sqlx::postgres::PgPoolOptions;
use std::env;

mod auth;
mod config;
mod db;
mod handlers;
mod models;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables from .env file
    dotenv().ok();
    
    // Initialize logging
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
    
    // Database connection string
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    
    // Create database connection pool
    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to create database connection pool");
    
    // Run database migrations
    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to run database migrations");
    
    // Get server address from environment
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let server_addr = format!("{}:{}", host, port);
    
    info!("Starting EvolveMe backend server at {}", server_addr);
    
    // Create and start HTTP server
    HttpServer::new(move || {
        // Configure CORS
        let cors = Cors::default()
            .allow_any_origin() // In production, restrict this to specific origins
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);
        
        App::new()
            // Add database pool to app state
            .app_data(actix_web::web::Data::new(db_pool.clone()))
            // Add middleware
            .wrap(middleware::Logger::default())
            .wrap(cors)
            // Register API routes
            .configure(handlers::auth::configure)
            .configure(handlers::health_data::configure)
    })
    .bind(server_addr)?
    .run()
    .await
}