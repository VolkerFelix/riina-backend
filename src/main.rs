use std::net::TcpListener;
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

use areum_backend::run;
use areum_backend::config::settings::{get_config, get_jwt_settings};
use areum_backend::telemetry::{get_subscriber, init_subscriber};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subscriber(
        "areum-backend".into(), "info".into(), std::io::stdout
    );
    init_subscriber(subscriber);

    // Panic if we can't read the config
    let config = get_config().expect("Failed to read the config.");
    // Get JWT settings
    let jwt_settings = get_jwt_settings(&config);
    // Only try to establish connection when actually used
    let conection_pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_secs(2))
        .connect_lazy(
            &config.database.connection_string().expose_secret()
        )
        .expect("Failed to create Postgres connection pool");
    let address = format!("{}:{}", config.application.host, config.application.port);
    let listener = TcpListener::bind(&address)?;
    
    run(listener, conection_pool, jwt_settings)?.await
}