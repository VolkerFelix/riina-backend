use std::net::TcpListener;
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

use evolveme_backend::run;
use evolveme_backend::config::settings::{get_config, get_jwt_settings, get_redis_url};
use evolveme_backend::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber(
        "evolveme-backend".into(), "info".into(), std::io::stdout
    );
    init_subscriber(subscriber);

    // Panic if we can't read the config
    let config = get_config().expect("Failed to read the config.");
    // JWT
    let jwt_settings = get_jwt_settings(&config);
    // Redis
    let redis_client = match redis::Client::open(get_redis_url(&config).expose_secret()) {
        Ok(client) => {
            tracing::info!("Redis client created successfully");
            println!("Redis client created successfully");
            Some(client)
        },
        Err(e) => {
            tracing::warn!("Failed to create Redis client: {}. Continuing without Redis.", e);
            eprintln!("Failed to create Redis client: {}", e);
            None
        }
    };
    // Only try to establish connection when actually used
    let conection_pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(2))
        .connect_lazy(
            &config.database.connection_string().expose_secret()
        )
        .expect("Failed to create Postgres connection pool");
    let address = format!("{}:{}", config.application.host, config.application.port);
    let listener = TcpListener::bind(&address)?;
    
    run(listener, conection_pool, jwt_settings, redis_client)?.await
}