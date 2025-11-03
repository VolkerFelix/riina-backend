use actix_web::{post, web, HttpResponse};
use sqlx::PgPool;

use crate::handlers::registration_handler::register_user;
use crate::models::user::RegistrationRequest;

#[post("/register_user")]
async fn register(
    user_form: web::Json<RegistrationRequest>,
    pool: web::Data<PgPool>,
    redis_client: web::Data<redis::Client>,
) -> HttpResponse {
    register_user(user_form, pool, redis_client).await
}