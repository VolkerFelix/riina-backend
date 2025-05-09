use secrecy::ExposeSecret;
use sqlx::{PgPool, PgConnection, Connection, Executor};
use std::net::TcpListener;
use uuid::Uuid;
use once_cell::sync::Lazy;

use areum_backend::run;
use areum_backend::config::settings::{get_config, DatabaseSettings, get_jwt_settings};
use areum_backend::telemetry::{get_subscriber, init_subscriber};

// Ensure that the `tracing` stack is only initialised once using `once_cell`
static TRACING: Lazy<()> = Lazy::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();

    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber(
            subscriber_name, 
            default_filter_level,
            std::io::stdout
        );
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber(
            subscriber_name, 
            default_filter_level,
            std::io::sink
        );
        init_subscriber(subscriber);
    }
});

pub struct TestApp{
    pub address: String,
    pub db_pool: PgPool
}

pub async fn spawn_app() -> TestApp {
    // The first time `initialize` is invoked the code in `TRACING` is executed.
    // All other invocations will instead skip execution.
    Lazy::force(&TRACING);

    let listener = TcpListener::bind("127.0.0.1:0")
        .expect("Failed to bind random port");
    // Get port assigned by the OS
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);
    let mut configuration = get_config().expect("Failed to read configuration.");
    configuration.database.db_name = Uuid::new_v4().to_string();
    let connection_pool = configure_db(&configuration.database)
        .await;
    let jwt_settings = get_jwt_settings(&configuration);
    let server = run(listener, connection_pool.clone(), jwt_settings)
        .expect("Failed to bind address");
    // Launch the server as a background task
    // tokio::spawn returns a handle to the spawned future,
    // but we have no use for it here, hence the non-binding let
    let _ = tokio::spawn(server);
    TestApp {
        address,
        db_pool: connection_pool
    }
}

pub async fn configure_db(config: &DatabaseSettings) -> PgPool {
    // Create database
    let mut connection = PgConnection::connect(
            &config.connection_string_without_db()
        )
        .await
        .expect("Failed to connect to Postgres");
    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.db_name).as_str())
        .await
        .expect("Failed to create database.");

    // Migrate database
    let connection_pool = PgPool::connect(&config.connection_string().expose_secret())
        .await
        .expect("Failed to connect to Postgres.");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate the database");

    connection_pool
}