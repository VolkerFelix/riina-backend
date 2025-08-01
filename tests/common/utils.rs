use secrecy::ExposeSecret;
use serde_json::json;
use sqlx::{PgPool, PgConnection, Connection, Executor};
use std::net::TcpListener;
use uuid::Uuid;
use once_cell::sync::Lazy;
use reqwest::Client;
use base64;
use chrono::{DateTime, Datelike, Duration, Utc, Weekday, NaiveTime};
use reqwest::Response;

use evolveme_backend::run;
use evolveme_backend::config::settings::{get_config, DatabaseSettings, get_jwt_settings, get_redis_url};
use evolveme_backend::telemetry::{get_subscriber, init_subscriber};
use evolveme_backend::services::SchedulerService;
use std::sync::Arc;

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

pub struct UserRegLoginResponse {
    pub token: String,
    pub user_id: Uuid,
    pub username: String,
}

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
    let redis_client = redis::Client::open(get_redis_url(&configuration).expose_secret())
        .ok();
    
    // Create scheduler service for tests
    let redis_client_arc = redis_client.as_ref().map(|client| Arc::new(client.clone()));
    let scheduler_service = Arc::new(
        SchedulerService::new_with_redis(connection_pool.clone(), redis_client_arc.clone())
            .await
            .expect("Failed to create scheduler service for tests")
    );
    
    let server = run(
        listener, 
        connection_pool.clone(), 
        jwt_settings,
        redis_client_arc,
        scheduler_service,
    )
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

pub fn parse_user_id_from_jwt_token(token: &str) -> Uuid {
    // Decode JWT to get user_id from the 'sub' claim
    let token_parts: Vec<&str> = token.split('.').collect();
    if token_parts.len() != 3 {
        panic!("Invalid JWT token format");
    }
    // Decode the payload (second part)
    let payload = base64::decode(token_parts[1])
        .expect("Failed to decode JWT payload");

    let payload_str = String::from_utf8(payload).expect("Failed to convert payload to string");
    let payload_json: serde_json::Value = serde_json::from_str(&payload_str)
        .expect("Failed to parse JWT payload");

    Uuid::parse_str(payload_json["sub"].as_str().expect("No 'sub' claim in JWT"))
        .expect("Failed to parse user ID from JWT")
}

pub async fn create_test_user_and_login(app_address: &str) -> UserRegLoginResponse {
    let client = Client::new();
    let username = format!("test_user_{}", &Uuid::new_v4().to_string()[..8]);
    let password = "password123";
    let email = format!("{}@example.com", username);

    let user_request = json!({
        "username": username,
        "password": password,
        "email": email
    });

    let _register_response = client
        .post(&format!("{}/register_user", app_address))
        .json(&user_request)
        .send()
        .await
        .expect("Failed to register user.");

        let login_request = json!({
            "username": username,
            "password": password
        });
    
        let login_response = client
            .post(&format!("{}/login", app_address))
            .json(&login_request)
            .send()
            .await
            .expect("Failed to execute login request.");

        let login_response: serde_json::Value = login_response.json().await.expect("Failed to parse login response");
        let token = login_response["token"].as_str().expect("No token in response");
        let user_id = parse_user_id_from_jwt_token(token);

        UserRegLoginResponse {
            token: token.to_string(),
            user_id: user_id,
            username: username
        }
}

/// Get the next occurrence of a specific weekday and time from now
pub fn get_next_date(day_of_week: Weekday, time: NaiveTime) -> DateTime<Utc> {
    let now = Utc::now();
    
    // Calculate days until the target weekday
    let current_weekday = now.weekday();
    let current_days = current_weekday.num_days_from_monday();
    let target_days = day_of_week.num_days_from_monday();
    
    let days_until_target = if current_days <= target_days {
        target_days - current_days
    } else {
        7 - (current_days - target_days)
    };
    
    // Get target date
    let target_date = now.date_naive() + Duration::days(days_until_target as i64);
    let target_datetime = target_date.and_time(time);
    let target_datetime_utc = DateTime::from_naive_utc_and_offset(target_datetime, Utc);
    
    // If it's already past this occurrence, get next week's occurrence
    if now >= target_datetime_utc {
        target_datetime_utc + Duration::weeks(1)
    } else {
        target_datetime_utc
    }
}

/// Helper function to make authenticated requests
pub async fn make_authenticated_request(
    client: &Client,
    method: reqwest::Method,
    url: &str,
    token: &str,
    body: Option<serde_json::Value>,
) -> Response {
    let mut request = client.request(method, url)
        .header("Authorization", format!("Bearer {}", token));

    if let Some(json_body) = body {
        request = request.json(&json_body);
    }

    request
        .send()
        .await
        .expect("Failed to execute request")
}