use tracing;
use uuid::Uuid;
use std::io::{Error, ErrorKind};

pub fn parse_user_id_from_jwt_token(token: &str) -> Result<Uuid, Error> {
    
    let user_id = match Uuid::parse_str(&token) {
        Ok(id) => {
            tracing::info!("User ID parsed successfully: {}", id);
            id
        },
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return Err(Error::new(ErrorKind::InvalidInput, "Invalid user ID"));
        }
    };
    Ok(user_id)
}