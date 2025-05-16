use actix::{Actor, ActorContext, AsyncContext, StreamHandler};
use actix_web_actors::ws;
use futures::StreamExt;
use std::time::{Duration, Instant};
use actix_web::web;
use crate::routes::websocket::messages::RedisMessage;
use tracing;

// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(120);

// WebSocket connection actor
pub struct WsConnection {
    heartbeat: Instant,
    user_id: String,
    redis: Option<web::Data<redis::Client>>,
}

impl Actor for WsConnection {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.heartbeat(ctx);
        self.send_welcome_message(ctx);
        self.setup_redis_subscription(ctx);
    }
}

impl WsConnection {
    pub fn new(user_id: String, redis: Option<web::Data<redis::Client>>) -> Self {
        Self {
            heartbeat: Instant::now(),
            user_id,
            redis,
        }
    }
    
    fn heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.heartbeat) > CLIENT_TIMEOUT {
                tracing::warn!("WebSocket client heartbeat missed, disconnecting!");
                ctx.stop();
                return;
            }
            
            tracing::debug!("Sending WebSocket heartbeat ping");
            ctx.ping(b"ping");
        });
    }

    fn send_welcome_message(&self, ctx: &mut ws::WebsocketContext<Self>) {
        let welcome_msg = serde_json::json!({
            "type": "welcome",
            "message": "WebSocket connection established",
            "user_id": self.user_id,
            "timestamp": chrono::Utc::now().to_rfc3339()
        });
    
        ctx.text(serde_json::to_string(&welcome_msg).unwrap_or_default());
    }

    fn setup_redis_subscription(&self, ctx: &mut ws::WebsocketContext<Self>) {
        let user_id = self.user_id.clone();
        let addr = ctx.address();

        if let Some(redis_client) = self.redis.clone() {            
            tokio::spawn(async move {
                match redis_client.get_async_connection().await {
                    Ok(conn) => {                        
                        let mut pubsub = conn.into_pubsub();
                        let channel = format!("evolveme:events:user:{}", user_id);
                        
                        match pubsub.subscribe(&channel).await {
                            Ok(_) => {
                                tracing::info!("Successfully subscribed to: {}", channel);                                
                                let global_channel = "evolveme:events:health_data";
                                let _ = pubsub.subscribe(global_channel).await;
                                
                                let mut stream = pubsub.on_message();
                                addr.do_send(RedisMessage(String::from("{\"test\":\"Redis subscription active!\"}")));
                                
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

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                tracing::debug!("Received ping from client");
                self.heartbeat = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                tracing::debug!("Received pong from client");
                self.heartbeat = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                tracing::debug!("Received text message: {}", text);
                
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    if json.get("type").and_then(|t| t.as_str()) == Some("ping") {
                        tracing::debug!("Received ping message from client");
                        self.heartbeat = Instant::now();
                        
                        let pong = serde_json::json!({
                            "type": "pong",
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        });
                        
                        ctx.text(serde_json::to_string(&pong).unwrap_or_default());
                        return;
                    }
                    
                    if json.get("type").and_then(|t| t.as_str()) == Some("pong") {
                        tracing::debug!("Received pong message from client");
                        self.heartbeat = Instant::now();
                        return;
                    }
                }
                
                let response = serde_json::json!({
                    "type": "echo",
                    "content": text.to_string(),
                    "timestamp": chrono::Utc::now().to_rfc3339()
                });
                
                ctx.text(serde_json::to_string(&response).unwrap_or_default());
            }
            Ok(ws::Message::Binary(bin)) => {
                ctx.binary(bin);
            }
            Ok(ws::Message::Close(reason)) => {
                tracing::info!("WebSocket closing with reason: {:?}", reason);
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

impl actix::Handler<RedisMessage> for WsConnection {
    type Result = ();
    
    fn handle(&mut self, msg: RedisMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
} 