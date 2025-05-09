// src/routes/protected.rs
use actix_web::{get, HttpResponse, web, HttpRequest};
use serde_json::json;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use secrecy::ExposeSecret;

use crate::config::jwt::JwtSettings;
use crate::middleware::auth::Claims;

#[get("/resource")]
async fn protected_resource(req: HttpRequest, jwt_settings: web::Data<JwtSettings>) -> HttpResponse {
    // Extract the token from the Authorization header
    let auth_header = req.headers().get("Authorization");
    if auth_header.is_none() {
        return HttpResponse::Unauthorized().json(json!({
            "status": "error",
            "message": "No authorization header provided"
        }));
    }

    let auth_str = auth_header.unwrap().to_str().unwrap_or("");
    if !auth_str.starts_with("Bearer ") {
        return HttpResponse::Unauthorized().json(json!({
            "status": "error",
            "message": "Invalid authorization format"
        }));
    }

    // Extract token without "Bearer " prefix
    let token = &auth_str[7..];

    // Validate the token
    let token_data = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_settings.secret.expose_secret().as_bytes()),
        &Validation::new(Algorithm::HS256)
    ) {
        Ok(data) => data,
        Err(e) => {
            return HttpResponse::Unauthorized().json(json!({
                "status": "error",
                "message": format!("Invalid token: {}", e)
            }));
        }
    };

    let claims = token_data.claims;

    // Return the protected resource with user data
    HttpResponse::Ok().json(json!({
        "status": "success",
        "message": "You have access to protected resource",
        "user_id": claims.sub,
        "username": claims.username
    }))
}