// Simple fix: Use your original connection.rs with better logging

use actix::{Actor, ActorContext, AsyncContext, StreamHandler, Handler};
use actix_web_actors::ws;
use futures::StreamExt;
use std::time::{Duration, Instant};
use actix_web::web;
use uuid::Uuid;
use chrono::Utc;
use tracing;

use crate::models::game_events::GameEvent;

// How often heartbeat pings are sent
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
// How long before lack of client response causes a timeout
const CLIENT_TIMEOUT: Duration = Duration::from_secs(120);

/// Game-focused WebSocket connection actor
pub struct GameConnection {
    heartbeat: Instant,
    user_id: Uuid,
    username: String,
    redis: Option<web::Data<redis::Client>>,
    session_id: Uuid,
}

impl Actor for GameConnection {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        tracing::info!("🔗 GameConnection started for user {} ({}) - session: {}", 
            self.user_id, self.username, self.session_id);
        
        self.heartbeat(ctx);
        self.send_connection_established(ctx);
        self.setup_game_event_subscription(ctx);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        tracing::info!("❌ GameConnection stopped for user {} ({}) - session: {}", 
            self.user_id, self.username, self.session_id);
        
        self.broadcast_player_left();
    }
}

impl GameConnection {
    pub fn new(user_id: Uuid, username: String, redis: Option<web::Data<redis::Client>>) -> Self {
        let session_id = Uuid::new_v4();
        tracing::info!("🆕 Creating new GameConnection for user {} ({}) - session: {}", 
            user_id, username, session_id);
        
        Self {
            heartbeat: Instant::now(),
            user_id,
            username,
            redis,
            session_id,
        }
    }
    
    fn heartbeat(&self, ctx: &mut ws::WebsocketContext<Self>) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.heartbeat) > CLIENT_TIMEOUT {
                tracing::warn!("💔 Game client heartbeat missed, disconnecting user: {} ({}) - session: {}", 
                    act.user_id, act.username, act.session_id);
                ctx.stop();
                return;
            }
            
            tracing::debug!("💓 Sending game client heartbeat ping for user: {} ({}) - session: {}", 
                act.user_id, act.username, act.session_id);
            ctx.ping(b"ping");
        });
    }

    fn send_connection_established(&self, ctx: &mut ws::WebsocketContext<Self>) {
        let connection_event = GameEvent::PlayerJoined {
            user_id: self.user_id,
            username: self.username.clone(),
            avatar_level: 1, // TODO: Get from database
            position: crate::models::game_events::Position { x: 0.0, y: 0.0 }, // TODO: Get from database
            timestamp: Utc::now(),
        };

        if let Ok(message) = serde_json::to_string(&connection_event) {
            ctx.text(message);
        }
    }

    fn setup_game_event_subscription(&self, ctx: &mut ws::WebsocketContext<Self>) {
        let user_id = self.user_id;
        let session_id = self.session_id;
        let username = self.username.clone();
        let addr = ctx.address();

        if let Some(redis_client) = self.redis.clone() {            
            tokio::spawn(async move {
                match redis_client.get_async_connection().await {
                    Ok(conn) => {                        
                        let mut pubsub = conn.into_pubsub();
                        
                        // Subscribe to multiple channels for different event types
                        let channels = vec![
                            format!("game:events:user:{}", user_id),           // User-specific events
                            "game:events:global".to_string(),                  // Global events (leaderboards, etc.)
                            "game:events:battles".to_string(),                 // Battle events
                            "game:events:territories".to_string(),             // Territory events
                        ];
                        
                        for channel in &channels {
                            if let Err(e) = pubsub.subscribe(channel).await {
                                tracing::error!("❌ Failed to subscribe to game channel {} for {} ({}): {}", 
                                    channel, user_id, username, e);
                                continue;
                            }
                            tracing::info!("📡 Subscribed to game events channel: {} for {} ({}) - session: {}", 
                                channel, user_id, username, session_id);
                        }
                        
                        let mut stream = pubsub.on_message();
                        
                        while let Some(msg) = stream.next().await {
                            match msg.get_payload::<String>() {
                                Ok(payload) => {
                                    tracing::debug!("📥 Received game event for {} ({}) session {}: {}", 
                                        user_id, username, session_id, payload);
                                    addr.do_send(GameEventMessage(payload));
                                },
                                Err(e) => {
                                    tracing::error!("❌ Failed to parse game event for {} ({}) session {}: {}", 
                                        user_id, username, session_id, e);
                                }
                            }
                        }
                    },
                    Err(e) => {
                        tracing::error!("❌ Failed to connect to Redis for game events for {} ({}) session {}: {}", 
                            user_id, username, session_id, e);
                    }
                }
            });
        } else {
            tracing::warn!("⚠️  No Redis client available for game events - real-time features disabled for {} ({}) session: {}", 
                user_id, username, session_id);
        }
    }

    fn broadcast_player_left(&self) {
        if let Some(redis_client) = &self.redis {
            let user_id = self.user_id;
            let username = self.username.clone();
            let redis_client = redis_client.clone();
            
            tokio::spawn(async move {
                if let Ok(mut conn) = redis_client.get_async_connection().await {
                    let leave_event = GameEvent::PlayerLeft {
                        user_id,
                        username,
                        timestamp: Utc::now(),
                    };
                    
                    if let Ok(message) = serde_json::to_string(&leave_event) {
                        let _: Result<i32, redis::RedisError> = redis::AsyncCommands::publish(
                            &mut conn, 
                            "game:events:global", 
                            message
                        ).await;
                    }
                }
            });
        }
    }
}

/// Message from Redis to WebSocket
#[derive(actix::Message)]
#[rtype(result = "()")]
pub struct GameEventMessage(pub String);

impl Handler<GameEventMessage> for GameConnection {
    type Result = ();
    
    fn handle(&mut self, msg: GameEventMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for GameConnection {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.heartbeat = Instant::now();
                ctx.pong(&msg);
                tracing::debug!("🏓 Pong sent for {} ({}) session: {}", 
                    self.user_id, self.username, self.session_id);
            }
            Ok(ws::Message::Pong(_)) => {
                self.heartbeat = Instant::now();
                tracing::debug!("🏓 Pong received for {} ({}) session: {}", 
                    self.user_id, self.username, self.session_id);
            }
            Ok(ws::Message::Text(text)) => {
                tracing::debug!("📨 Received game message from {} ({}) session {}: {}", 
                    self.user_id, self.username, self.session_id, text);
                self.heartbeat = Instant::now();
                
                // Handle incoming game commands
                self.handle_game_message(&text, ctx);
            }
            Ok(ws::Message::Binary(_)) => {
                tracing::warn!("⚠️  Received unexpected binary message from {} ({}) session: {}", 
                    self.user_id, self.username, self.session_id);
            }
            Ok(ws::Message::Close(reason)) => {
                tracing::info!("🔒 Game WebSocket closing for {} ({}) session {}: {:?}", 
                    self.user_id, self.username, self.session_id, reason);
                ctx.close(reason);
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

impl GameConnection {
    fn handle_game_message(&self, message: &str, ctx: &mut ws::WebsocketContext<Self>) {
        // Parse incoming game commands from client
        if let Ok(command) = serde_json::from_str::<serde_json::Value>(message) {
            match command.get("type").and_then(|t| t.as_str()) {
                Some("ping") => {
                    let pong = serde_json::json!({
                        "type": "pong",
                        "timestamp": Utc::now().to_rfc3339(),
                        "session_id": self.session_id
                    });
                    ctx.text(serde_json::to_string(&pong).unwrap_or_default());
                    tracing::debug!("🏓 App-level pong sent for {} ({}) session: {}", 
                        self.user_id, self.username, self.session_id);
                }
                Some("avatar_position_update") => {
                    // Handle avatar position updates
                    if let (Some(x), Some(y)) = (
                        command.get("x").and_then(|v| v.as_f64()),
                        command.get("y").and_then(|v| v.as_f64())
                    ) {
                        self.handle_position_update(x, y);
                    }
                }
                Some("battle_strategy") => {
                    // Handle battle strategy selection
                    if let Some(strategy) = command.get("strategy").and_then(|s| s.as_str()) {
                        self.handle_battle_strategy(strategy);
                    }
                }
                Some("request_leaderboard") => {
                    // Handle leaderboard requests
                    self.handle_leaderboard_request(ctx);
                }
                _ => {
                    tracing::debug!("❓ Unknown game command from {} ({}) session {}: {}", 
                        self.user_id, self.username, self.session_id, message);
                }
            }
        }
    }
    
    fn handle_position_update(&self, x: f64, y: f64) {
        // Broadcast position update to other players
        if let Some(redis_client) = &self.redis {
            let user_id = self.user_id;
            let username = self.username.clone();
            let redis_client = redis_client.clone();
            
            tokio::spawn(async move {
                if let Ok(mut conn) = redis_client.get_async_connection().await {
                    // TODO: Get actual stats from database
                    let update_event = GameEvent::AvatarUpdated {
                        user_id,
                        username,
                        stats: crate::models::game_events::AvatarStats {
                            stamina: 50,
                            strength: 50,
                            wisdom: 50,
                            mana: 50,
                            experience_points: 1000,
                        },
                        level: 1,
                        position: crate::models::game_events::Position { x, y },
                        timestamp: Utc::now(),
                    };
                    
                    if let Ok(message) = serde_json::to_string(&update_event) {
                        let _: Result<i32, redis::RedisError> = redis::AsyncCommands::publish(
                            &mut conn, 
                            "game:events:global", 
                            message
                        ).await;
                    }
                }
            });
        }
    }
    
    fn handle_battle_strategy(&self, strategy: &str) {
        tracing::info!("⚔️  User {} ({}) session {} selected battle strategy: {}", 
            self.user_id, self.username, self.session_id, strategy);
        // TODO: Implement battle strategy handling
    }
    
    fn handle_leaderboard_request(&self, ctx: &mut ws::WebsocketContext<Self>) {
        // TODO: Get actual leaderboard from database
        let mock_leaderboard = GameEvent::LeaderboardUpdate {
            daily_rankings: vec![
                crate::models::game_events::PlayerRanking {
                    user_id: Uuid::new_v4(),
                    username: "GameMaster".to_string(),
                    avatar_level: 15,
                    total_stats: 300,
                    rank: 1,
                    score: 5000,
                },
                crate::models::game_events::PlayerRanking {
                    user_id: self.user_id,
                    username: self.username.clone(),
                    avatar_level: 1,
                    total_stats: 200,
                    rank: 2,
                    score: 1000,
                },
            ],
            updated_at: Utc::now(),
        };
        
        if let Ok(message) = serde_json::to_string(&mock_leaderboard) {
            ctx.text(message);
        }
    }
}