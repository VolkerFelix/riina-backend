mod connection;
mod messages;
mod auth;

use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use crate::middleware::auth::Claims;
use crate::config::jwt::JwtSettings;
use tracing;

pub use connection::WsConnection;
pub use messages::TokenQuery;
pub use auth::decode_token;

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