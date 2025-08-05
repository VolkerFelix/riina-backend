use std::net::TcpListener;
use std::sync::Arc;
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

use evolveme_backend::run;
use evolveme_backend::config::settings::{get_config, get_jwt_settings, get_redis_url};
use evolveme_backend::telemetry::{get_subscriber, init_subscriber};
use evolveme_backend::services::SchedulerService;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Panic if we can't read the config
    let config = get_config().expect("Failed to read the config.");
    
    let subscriber = get_subscriber(
        "evolveme-backend".into(), 
        config.application.log_level.clone(), 
        std::io::stdout
    );
    init_subscriber(subscriber);

    // JWT
    let jwt_settings = get_jwt_settings(&config);
    // Redis
    let redis_client_raw = match redis::Client::open(get_redis_url(&config).expose_secret()) {
        Ok(client) => {
            tracing::info!("Redis client created successfully");
            Some(client)
        },
        Err(e) => {
            tracing::error!("Failed to create Redis client: {}. LLM features will not work properly.", e);
            eprintln!("Failed to create Redis client: {}", e);
            eprintln!("Redis is required for LLM integration. Please ensure Redis is running.");
            std::process::exit(1);
        }
    };
    // Create Arc version to be thread safe
    let redis_client_arc = redis_client_raw.map(|client| Arc::new(client));
    // Only try to establish connection when actually used
    let conection_pool = PgPoolOptions::new()
        .max_connections(32)
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(600))
        .max_lifetime(Duration::from_secs(1800))
        .connect_lazy(
            &config.database.connection_string().expose_secret()
        )
        .expect("Failed to create Postgres connection pool");
    let address = format!("{}:{}", config.application.host, config.application.port);
    let listener = TcpListener::bind(&address)?;
    
    // Initialize the scheduler service
    let scheduler_service = match SchedulerService::new_with_redis(conection_pool.clone(), redis_client_arc.clone()).await {
        Ok(scheduler) => {
            match scheduler.start().await {
                Ok(_) => {
                    tracing::info!("✅ Scheduler service started successfully");
                    Arc::new(scheduler)
                }
                Err(e) => {
                    tracing::error!("❌ Failed to start scheduler: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Failed to create scheduler service: {}", e);
            std::process::exit(1);
        }
    };
    
    run(
        listener,
        conection_pool,
        jwt_settings,
        redis_client_arc,
        scheduler_service
    )?.await
}