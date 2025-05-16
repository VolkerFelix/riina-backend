use actix;
use serde::Deserialize;

// Message from Redis to WebSocket
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct RedisMessage(pub String);

// Query parameter struct for token
#[derive(Deserialize)]
pub struct TokenQuery {
    pub token: String,
} 