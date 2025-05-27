use std::net::TcpListener;
use secrecy::ExposeSecret;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;

use evolveme_backend::run;
use evolveme_backend::config::settings::{get_config, get_jwt_settings, get_redis_url};
use evolveme_backend::telemetry::{get_subscriber, init_subscriber};
use evolveme_backend::services::llm_service::LLMService;
use evolveme_backend::services::conversation_service::ConversationService;

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
    let redis_client = match redis::Client::open(get_redis_url(&config).expose_secret()) {
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
    // Initialize conversation service
    let conversation_service = ConversationService::new(redis_client.clone().unwrap());
    // Initialize LLM service
    let llm_service = if config.llm.enabled {
        tracing::info!("Initializing LLM service at: {}", config.llm.service_url);
        let service = LLMService::new(config.llm.service_url.clone());
        
        // Test LLM service health
        if service.health_check().await {
            tracing::info!("LLM service health check passed");
        } else {
            tracing::warn!("LLM service health check failed - will use fallback responses");
        }
        service
    } else {
        tracing::info!("LLM service disabled in configuration");
        LLMService::new("http://disabled".to_string())
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
    
    run(
        listener,
        conection_pool,
        jwt_settings,
        llm_service,
        conversation_service,
        redis_client
    )?.await
}