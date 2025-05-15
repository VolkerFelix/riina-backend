use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use std::time::{Duration, Instant};
use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use uuid::Uuid;
use serde::Deserialize;
use crate::middleware::auth::Claims;
use futures_util::StreamExt;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use secrecy::ExposeSecret;
use crate::config::jwt::JwtSettings;

// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(30);

// Query parameter struct for token
#[derive(Deserialize)]
pub struct TokenQuery {
    token: String,
}

// WebSocket connection actor
struct WsConnection {
    heartbeat: Instant,
    user_id: String,
    redis: Option<web::Data<redis::Client>>,
}

impl Actor for WsConnection {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.heartbeat(ctx);
        
        // Redis subscription setup
        let user_id = self.user_id.clone();
        let addr = ctx.address();

        // Check if Redis client is available
        if let Some(redis_client) = self.redis.clone() {            
            // Launch Redis subscriber in separate task
            tokio::spawn(async move {
                // Connect to Redis
                match redis_client.get_async_connection().await {
                    Ok(conn) => {                        
                        // Create PubSub
                        let mut pubsub = conn.into_pubsub();
                        let channel = format!("evolveme:events:user:{}", user_id);
                        // Subscribe to user channel
                        match pubsub.subscribe(&channel).await {
                            Ok(_) => {
                                tracing::info!("Successfully subscribed to: {}", channel);                                
                                // Subscribe to global channel too
                                let global_channel = "evolveme:events:health_data";
                                let _ = pubsub.subscribe(global_channel).await;
                                
                                // Process messages
                                let mut stream = pubsub.on_message();
                                
                                // Send a test message to the WebSocket to confirm it's working
                                addr.do_send(RedisMessage(String::from("{\"test\":\"Redis subscription active!\"}")));
                                
                                // Process actual Redis messages
                                while let Some(msg) = stream.next().await {
                                    match msg.get_payload::<String>() {
                                        Ok(payload) => {
                                            tracing::info!("Received Redis message: {}", payload);
                                            addr.do_send(RedisMessage(payload));
                                        },
                                        Err(e) => {
                                            tracing::error!("Failed to parse Redis message: {}", e);
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                tracing::error!("Failed to subscribe to channel {}: {}", channel, e);
                            }
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to connect to Redis: {}", e);
                    }
                }
            });
        } else {
            tracing::error!("No Redis client available for WebSocket - check your app configuration!");
        }
    }
}

// Message from Redis to WebSocket
#[derive(actix::Message)]
#[rtype(result = "()")]
struct RedisMessage(String);

// Handle Redis messages
impl actix::Handler<RedisMessage> for WsConnection {
    type Result = ();
    
    fn handle(&mut self, msg: RedisMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

// Handle WebSocket messages
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.heartbeat = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.heartbeat = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                tracing::info!("WebSocket text message: {}", text);
                ctx.text(format!("Echo: {}", text));
            }
            Ok(ws::Message::Binary(bin)) => {
                ctx.binary(bin);
            }
            Ok(ws::Message::Close(reason)) => {
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

impl WsConnection {
    fn new(user_id: String, redis: Option<web::Data<redis::Client>>) -> Self {
        Self {
            heartbeat: Instant::now(),
            user_id,
            redis,
        }
    }
    
    fn heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.heartbeat) > CLIENT_TIMEOUT {
                tracing::error!("WebSocket heartbeat failed, disconnecting!");
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }
}

// Helper function to decode JWT token
fn decode_token(token: &str, jwt_settings: &web::Data<JwtSettings>) -> Result<Claims, jsonwebtoken::errors::Error> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_settings.secret.expose_secret().as_bytes()),
        &Validation::new(Algorithm::HS256)
    ).map(|data| data.claims)
}

// WebSocket route handler that supports both Authorization header and query parameter
pub async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    query: Option<web::Query<TokenQuery>>,
    claims: Option<web::ReqData<Claims>>,
    redis: Option<web::Data<redis::Client>>,
    jwt_settings: web::Data<JwtSettings>,
) -> Result<HttpResponse, Error> {
    tracing::info!("New WebSocket connection request");
    
    // Try to get user_id from different sources
    let user_id = if let Some(claims) = claims {
        // JWT from Authorization header via middleware
        tracing::info!("Using JWT from Authorization header");
        claims.sub.clone()
    } else if let Some(query) = query {
        // JWT from query parameter
        tracing::info!("Using JWT from query parameter");
        match decode_token(&query.token, &jwt_settings) {
            Ok(token_claims) => {
                tracing::info!("JWT from query parameter verified for user: {}", token_claims.username);
                token_claims.sub
            },
            Err(e) => {
                tracing::error!("Invalid JWT in query parameter: {}", e);
                return Err(actix_web::error::ErrorUnauthorized("Invalid token"));
            }
        }
    } else {
        // No authentication provided
        tracing::error!("No authentication provided");
        return Err(actix_web::error::ErrorUnauthorized("No authentication"));
    };
    
    // Start WebSocket connection
    let resp = ws::start(
        WsConnection::new(user_id, redis),
        &req,
        stream,
    )?;
    
    tracing::info!("WebSocket connection established");
    Ok(resp)
}