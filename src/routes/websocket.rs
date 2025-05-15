// src/routes/websocket.rs
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use futures::StreamExt;
use std::time::{Duration, Instant};
use actix::{Actor, ActorContext, AsyncContext, Handler, StreamHandler, WrapFuture};
use actix::prelude::*;
use uuid::Uuid;
use crate::middleware::auth::Claims;

// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(30);

/// WebSocket connection is represented by `WsConnection` actor
#[allow(dead_code)]
struct WsConnection {
    /// Client must send ping at least once per 10 seconds,
    /// otherwise we drop connection
    heartbeat: Instant,
    /// Unique session id
    id: String,
    /// User id from JWT claims
    user_id: String,
    /// Redis client for pub/sub
    redis: Option<web::Data<redis::Client>>,
}

impl Actor for WsConnection {
    type Context = ws::WebsocketContext<Self>;

    /// Method is called on actor start
    fn started(&mut self, ctx: &mut Self::Context) {
        self.heartbeat(ctx);
        
        // Start listening for Redis messages if Redis is available
        if let Some(redis_client) = &self.redis {
            let redis = redis_client.clone();
            let user_id = self.user_id.clone();
            let addr = ctx.address();
            
            // Create a separate Redis connection for subscribing
            let fut = async move {
                match redis.get_async_connection().await {
                    Ok(con) => {
                        // Subscribe to user-specific channel and global events
                        let mut pubsub = con.into_pubsub();
                        let channel = format!("evolveme:events:user:{}", user_id);
                        
                        if let Err(e) = pubsub.subscribe(&channel).await {
                            tracing::error!("Failed to subscribe to Redis channel {}: {}", channel, e);
                            return;
                        }
                        
                        // Also subscribe to global health data events
                        if let Err(e) = pubsub.subscribe("evolveme:events:health_data").await {
                            tracing::error!("Failed to subscribe to Redis global channel: {}", e);
                            return;
                        }
                        
                        tracing::info!("WebSocket subscribed to Redis channels for user {}", user_id);
                        
                        // Listen for published messages
                        let mut msg_stream = pubsub.on_message();
                        
                        // Process incoming Redis messages
                        while let Some(msg) = msg_stream.next().await {
                            let payload: String = msg.get_payload().unwrap_or_default();
                            tracing::debug!("Received Redis message: {}", payload);
                            
                            // Send message to actor using Addr
                            addr.do_send(RedisMessage(payload));
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to get Redis connection for WebSocket: {}", e);
                    }
                }
            };
            
            // Spawn the future into the Actor's context
            ctx.spawn(fut.into_actor(self));
        }
    }
}

/// Handler for WebSocket messages
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
                tracing::debug!("WS Text message: {}", text);
                // Implement command handling logic here if needed
                // For now, we just echo the message back
                ctx.text(format!("Echo: {}", text));
            }
            Ok(ws::Message::Binary(bin)) => {
                tracing::debug!("WS Binary message: {:?}", bin);
                ctx.binary(bin);
            }
            Ok(ws::Message::Close(reason)) => {
                tracing::info!("WebSocket closed with reason: {:?}", reason);
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
            id: Uuid::new_v4().to_string(),
            user_id,
            redis,
        }
    }
    
    /// Heartbeat to check for disconnection
    fn heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // If heartbeat is older than CLIENT_TIMEOUT, disconnect
            if Instant::now().duration_since(act.heartbeat) > CLIENT_TIMEOUT {
                tracing::info!("WebSocket client timed out, disconnecting!");
                ctx.stop();
                return;
            }
            
            // Send ping
            ctx.ping(b"");
        });
    }
}

/// WebSocket route handler
pub async fn ws_route(
    req: HttpRequest,
    stream: web::Payload,
    claims: web::ReqData<Claims>,
    redis: Option<web::Data<redis::Client>>,
) -> Result<HttpResponse, Error> {
    tracing::info!("New WebSocket connection from user: {}", claims.username);
    
    // Create websocket connection
    let resp = ws::start(
        WsConnection::new(claims.sub.clone(), redis),
        &req,
        stream,
    )?;
    
    Ok(resp)
}

// Add this struct and impl before the WsConnection struct
#[derive(Message)]
#[rtype(result = "()")]
struct RedisMessage(String);

impl Handler<RedisMessage> for WsConnection {
    type Result = ();

    fn handle(&mut self, msg: RedisMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}