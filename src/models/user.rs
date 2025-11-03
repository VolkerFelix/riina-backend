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