
use actix_web::{web, HttpResponse, Responder};
use bcrypt::{hash, verify, DEFAULT_COST};
use sqlx::{Pool, Postgres};
use validator::Validate;

use crate::auth::jwt::generate_token;
use crate::db::health_data::{get_user_by_email, create_user};
use crate::models::user::{AuthResponse, LoginUserDto, RegisterUserDto, UserResponse};

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .route("/register", web::post().to(register))
            .route("/login", web::post().to(login)),
    );
}

async fn register(
    pool: web::Data<Pool<Postgres>>,
    data: web::Json<RegisterUserDto>,
) -> impl Responder {
    // Validate input data
    if let Err(e) = data.validate() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": format!("Validation error: {}", e),
        }));
    }
    
    // Check if user with this email already exists
    let existing_user = get_user_by_email(&pool, &data.email).await;
    if let Ok(Some(_)) = existing_user {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "status": "error",
            "message": "User with this email already exists",
        }));
    }
    
    // Hash password
    let password_hash = match hash(&data.password, DEFAULT_COST) {
        Ok(hash) => hash,
        Err(_) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "message": "Failed to hash password",
            }));
        }
    };
    
    // Create user in database
    let user_result = create_user(&pool, &data.email, &data.username, &password_hash).await;
    let user = match user_result {
        Ok(user) => user,
        Err(e) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "message": format!("Failed to create user: {}", e),
            }));
        }
    };
    
    // Generate JWT token
    let token = match generate_token(user.id) {
        Ok(token) => token,
        Err(_) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "message": "Failed to generate token",
            }));
        }
    };
    
    // Prepare response
    let user_response = UserResponse::from(user);
    let auth_response = AuthResponse {
        user: user_response,
        token,
    };
    
    HttpResponse::Created().json(auth_response)
}

async fn login(
    pool: web::Data<Pool<Postgres>>,
    data: web::Json<LoginUserDto>,
) -> impl Responder {
    // Get user by email
    let user_result = get_user_by_email(&pool, &data.email).await;
    let user = match user_result {
        Ok(Some(user)) => user,
        Ok(None) => {
            return HttpResponse::Unauthorized().json(serde_json::json!({
                "status": "error",
                "message": "Invalid email or password",
            }));
        }
        Err(_) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "message": "Failed to query user",
            }));
        }
    };
    
    // Verify password
    let is_valid = match verify(&data.password, &user.password_hash) {
        Ok(valid) => valid,
        Err(_) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "message": "Failed to verify password",
            }));
        }
    };
    
    if !is_valid {
        return HttpResponse::Unauthorized().json(serde_json::json!({
            "status": "error",
            "message": "Invalid email or password",
        }));
    }
    
    // Generate JWT token
    let token = match generate_token(user.id) {
        Ok(token) => token,
        Err(_) => {
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "status": "error",
                "message": "Failed to generate token",
            }));
        }
    };
    
    // Prepare response
    let user_response = UserResponse::from(user);
    let auth_response = AuthResponse {
        user: user_response,
        token,
    };
    
    HttpResponse::Ok().json(auth_response)
}