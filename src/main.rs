use std::net::TcpListener;
use std::sync::Arc;
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

use riina_backend::run;
use riina_backend::config::settings::{get_config, get_jwt_settings};
use riina_backend::services::{SchedulerService, MinIOService, telemetry::{get_subscriber, init_subscriber}, redis_service::RedisService};
use riina_backend::ml_client::MLClient;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Panic if we can't read the config
    let config = get_config().expect("Failed to read the config.");
    // Telemetry
    let subscriber = get_subscriber(
        "riina-backend".into(), 
        config.application.log_level.clone(), 
        std::io::stdout
    );
    init_subscriber(subscriber);

    // JWT
    let jwt_settings = get_jwt_settings(&config);
    // Redis
    let redis_service = RedisService::new(&config.redis).await.expect("Failed to create Redis service");
    // MinIO
    let minio_service = MinIOService::new(&config.minio).await.expect("Failed to create MinIO service");
    // Postgres
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
    
    // Scheduler service
    let scheduler_service = match SchedulerService::new_with_redis(conection_pool.clone(), redis_service.client.clone()).await {
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

    // ML Client
    let client = MLClient::new(config.ml.service_url.clone());
    let ml_client = match client.health_check().await {
        Ok(true) => {
            tracing::info!("✅ ML service is healthy at {}", config.ml.service_url);
            Some(client)
        }
        Ok(false) | Err(_) => {
            tracing::warn!("⚠️ ML service unavailable. Continuing without ML classification.");
            None
        }
    };

    run(
        listener,
        conection_pool,
        jwt_settings,
        redis_service.client,
        scheduler_service,
        minio_service,
        ml_client
    )?.await
}