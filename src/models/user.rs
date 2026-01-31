use std::fmt;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use secrecy::SecretString;
use sqlx::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[derive(Default)]
pub enum UserRole {
    #[sqlx(rename = "superadmin")]
    SuperAdmin,
    #[sqlx(rename = "admin")]
    Admin,
    #[sqlx(rename = "moderator")]
    Moderator,
    #[sqlx(rename = "user")]
    #[default]
    User,
}


impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserRole::SuperAdmin => write!(f, "superadmin"),
            UserRole::Admin => write!(f, "admin"),
            UserRole::Moderator => write!(f, "moderator"),
            UserRole::User => write!(f, "user"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[sqlx(type_name = "varchar", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum UserStatus {
    #[sqlx(rename = "active")]
    #[default]
    Active,
    #[sqlx(rename = "inactive")]
    Inactive,
    #[sqlx(rename = "suspended")]
    Suspended,
    #[sqlx(rename = "banned")]
    Banned,
}


impl fmt::Display for UserStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserStatus::Active => write!(f, "active"),
            UserStatus::Inactive => write!(f, "inactive"),
            UserStatus::Suspended => write!(f, "suspended"),
            UserStatus::Banned => write!(f, "banned"),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub username: String,
    #[serde(serialize_with = "serialize_secret_string", deserialize_with = "deserialize_secret_string")]
    pub password_hash: SecretString,
    pub role: UserRole,
    pub status: UserStatus,
    pub profile_picture_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct RegistrationRequest {
    pub username: String,
    pub email: String,
    #[serde(serialize_with = "serialize_secret_string", deserialize_with = "deserialize_secret_string")]
    pub password: SecretString,
}

impl RegistrationRequest {
    /// Validate the registration request
    pub fn validate(&self) -> Result<(), String> {
        // Validate username
        self.validate_username()?;

        // Validate email
        if self.email.is_empty() {
            return Err("Email cannot be empty".to_string());
        }

        // Basic email format validation
        if !self.email.contains('@') {
            return Err("Invalid email format".to_string());
        }

        Ok(())
    }

    /// Validate username format
    /// - Only alphanumeric characters, underscores, and hyphens allowed
    /// - No spaces, no special characters (é, õ, etc.)
    /// - Length between 3 and 30 characters
    /// - Cannot start or end with underscore or hyphen
    pub fn validate_username(&self) -> Result<(), String> {
        let username = self.username.trim();

        // Check length
        if username.len() < 3 {
            return Err("Username must be at least 3 characters long".to_string());
        }

        if username.len() > 30 {
            return Err("Username cannot exceed 30 characters".to_string());
        }

        // Check for invalid characters (only allow a-z, A-Z, 0-9, _, -)
        if !username.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
            return Err("Username can only contain letters, numbers, underscores, and hyphens (no spaces or special characters like é, õ)".to_string());
        }

        // Cannot start or end with underscore or hyphen
        if username.starts_with('_') || username.starts_with('-') {
            return Err("Username cannot start with an underscore or hyphen".to_string());
        }

        if username.ends_with('_') || username.ends_with('-') {
            return Err("Username cannot end with an underscore or hyphen".to_string());
        }

        // Check for consecutive underscores or hyphens
        if username.contains("__") || username.contains("--") {
            return Err("Username cannot contain consecutive underscores or hyphens".to_string());
        }

        Ok(())
    }
}

impl std::fmt::Display for RegistrationRequest{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Username: {}, Email: {}", self.username, self.email)
    }
}

pub fn serialize_secret_string<S>(_: &SecretString, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str("[REDACTED]")
}

pub fn deserialize_secret_string<'de, D>(deserializer: D) -> Result<SecretString, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(SecretString::new(s.into_boxed_str()))
}