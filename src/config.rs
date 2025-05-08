use std::env;

pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expiration_hours: i64,
    pub host: String,
    pub port: String,
}

impl Config {
    pub fn from_env() -> Self {
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let jwt_secret = env::var("JWT_SECRET").expect("JWT_SECRET must be set");
        let jwt_expiration_hours = env::var("JWT_EXPIRATION_HOURS")
            .unwrap_or_else(|_| "24".to_string())
            .parse()
            .unwrap_or(24);
        let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
        let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());
        
        Config {
            database_url,
            jwt_secret,
            jwt_expiration_hours,
            host,
            port,
        }
    }
}