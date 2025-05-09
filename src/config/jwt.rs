use secrecy::SecretString;

#[derive(Debug)]
pub struct JwtSettings {
    pub secret: SecretString,
    pub expiration_hours: i64,
}

impl JwtSettings {
    pub fn new(secret: String, expiration_hours: i64) -> Self {
        Self {
            secret: SecretString::new(secret.into_boxed_str()),
            expiration_hours,
        }
    }
}