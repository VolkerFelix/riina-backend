use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, FromRow, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub username: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Validate, Deserialize)]
pub struct RegisterUserDto {
    #[validate(email)]
    pub email: String,
    
    #[validate(length(min = 3, max = 30))]
    pub username: String,
    
    #[validate(length(min = 8, message = "Password must be at least 8 characters"))]
    pub password: String,
}

#[derive(Debug, Validate, Deserialize)]
pub struct LoginUserDto {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub username: String,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        UserResponse {
            id: user.id,
            email: user.email,
            username: user.username,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub user: UserResponse,
    pub token: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenClaims {
    pub sub: String, // user id
    pub exp: usize,  // expiration time
    pub iat: usize,  // issued at
}