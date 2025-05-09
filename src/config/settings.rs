use std::env;
use config::{Config, File, ConfigError};
use dotenv::dotenv;
use secrecy::{ExposeSecret, SecretString};

use crate::config::jwt::JwtSettings;

#[derive(serde::Deserialize, Debug)]
pub struct Settings{
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
    pub jwt: JwtConfig
}

#[derive(serde::Deserialize, Debug)]
pub struct JwtConfig {
    pub secret: SecretString,
    pub expiration_hours: i64,
}

#[derive(serde::Deserialize, Debug)]
pub struct DatabaseSettings{
    pub user: String,
    pub password: SecretString,
    pub port: u16,
    pub host: String,
    pub db_name: String,
    #[serde(default)]
    pub db_url: Option<SecretString>
}

impl DatabaseSettings {
    pub fn connection_string(&self) -> SecretString {
        match &self.db_url {
            Some(db_url) => db_url.clone(),
            None => {
                SecretString::new(format!(
                    "postgres://{}:{}@{}:{}/{}",
                    self.user, self.password.expose_secret(), self.host, self.port, self.db_name
                ).into_boxed_str())
            }
        }
    }

    pub fn connection_string_without_db(&self) -> String {
        format!(
            "postgres://{}:{}@{}:{}",
            self.user, self.password.expose_secret(), self.host, self.port
        )
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct ApplicationSettings{
    pub user: String,
    pub password: SecretString, 
    pub port: u16,
    pub host: String,
    pub log_level: String
}

pub fn get_config() -> Result<Settings, ConfigError> {
    let base_path = std::env::current_dir()
        .expect("Failed to determine the current directory");
    let configuration_directory = base_path.join("configuration");
    
    dotenv().ok();

    let environment: Environment = env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.");

    let env_filename = format!("{}.yml", environment.as_str());
    let config = Config::builder()
        .add_source(File::from(configuration_directory.join("base.yml")))
        .add_source(File::from(configuration_directory.join(env_filename)))
        .add_source(
            config::Environment::default()
                .prefix("POSTGRES")
                .prefix_separator("__")
                .separator("__")
        )
        .add_source(
            config::Environment::default()
                .prefix("APP")
                .prefix_separator("__")
                .separator("__")
        )
        .build()?;

    let mut settings = config.try_deserialize::<Settings>()?;

    // In Fly.io the DATABASE_URL is directly exposed as an env var
    if let Ok(db_url) = env::var("DATABASE_URL") {
        settings.database.db_url = Some(SecretString::new(db_url.into_boxed_str()));
    }

    // Allow JWT secret override from environment variable
    if let Ok(jwt_secret) = env::var("JWT_SECRET") {
        settings.jwt.secret = SecretString::new(jwt_secret.into_boxed_str());
    }

    Ok(settings)
}

pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. \
                Use either `local` or `production`.",
                other
            )),
        }
    }
}

pub fn get_jwt_settings(settings: &Settings) -> JwtSettings {
    JwtSettings::new(
        settings.jwt.secret.expose_secret().to_string().clone(),
        settings.jwt.expiration_hours,
    )
}