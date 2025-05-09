use actix_web::{get, Responder};

use crate::handlers::backend_health_handler::backend_health_check;

#[get("/backend_health")]
async fn backend_health() -> impl Responder {
    backend_health_check().await
}