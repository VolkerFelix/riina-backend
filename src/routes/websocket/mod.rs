mod connection;
mod messages;
mod auth;

use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use crate::middleware::auth::Claims;
use crate::config::jwt::JwtSettings;
use uuid::Uuid;
use tracing;

pub use connection::GameConnection;
pub use messages::TokenQuery;
pub use auth::decode_token;

/// Game-focused WebSocket route handler
pub async fn game_ws_route(
    req: HttpRequest,
    stream: web::Payload,
    query: Option<web::Query<TokenQuery>>,
    claims: Option<web::ReqData<Claims>>,
    redis: Option<web::Data<redis::Client>>,
    jwt_settings: web::Data<JwtSettings>,
) -> Result<HttpResponse, Error> {
    tracing::info!("New game WebSocket connection request");
    
    // Try to get user info from different sources
    let (user_id, username) = if let Some(claims) = claims {
        // JWT from Authorization header via middleware
        tracing::info!("Using JWT from Authorization header for user: {}", claims.username);
        (claims.sub.clone(), claims.username.clone())
    } else if let Some(query) = query {
        // JWT from query parameter
        tracing::info!("Using JWT from query parameter");
        match decode_token(&query.token, &jwt_settings) {
            Ok(token_claims) => {
                tracing::info!("JWT from query parameter verified for user: {}", token_claims.username);
                (token_claims.sub, token_claims.username)
            },
            Err(e) => {
                tracing::error!("Invalid JWT in query parameter: {}", e);
                return Err(actix_web::error::ErrorUnauthorized("Invalid token"));
            }
        }
    } else {
        // No authentication provided
        tracing::error!("No authentication provided for game WebSocket");
        return Err(actix_web::error::ErrorUnauthorized("No authentication"));
    };
    
    // Parse user_id as UUID
    let user_uuid = match Uuid::parse_str(&user_id) {
        Ok(uuid) => uuid,
        Err(e) => {
            tracing::error!("Invalid user ID format: {}", e);
            return Err(actix_web::error::ErrorBadRequest("Invalid user ID"));
        }
    };
    
    // Start game WebSocket connection
    let resp = ws::start(
        GameConnection::new(user_uuid, username, redis),
        &req,
        stream,
    )?;
    
    tracing::info!("Game WebSocket connection established for user: {}", user_uuid);
    Ok(resp)
}