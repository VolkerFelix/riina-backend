use redis::Client as RedisClient;
use secrecy::ExposeSecret;
use riina_backend::config::settings::get_config;

/// Helper function to setup Redis pubsub connection for a specific channel
pub async fn setup_redis_pubsub(channel: &str) -> redis::aio::PubSub {
    let config = get_config().expect("Failed to read config");
    let redis_url = format!("redis://:{}@localhost:{}",
        config.redis.password.expose_secret(),
        config.redis.port
    );
    let redis_client = RedisClient::open(redis_url).expect("Failed to create Redis client");
    let pubsub_conn = redis_client
        .get_async_connection()
        .await
        .expect("Failed to create pubsub connection");
    let mut pubsub = pubsub_conn.into_pubsub();
    pubsub
        .subscribe(channel)
        .await
        .unwrap_or_else(|_| panic!("Failed to subscribe to {}", channel));
    pubsub
}
