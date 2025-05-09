use actix_web::HttpResponse;
use serde_json::json;

pub async fn backend_health_check() -> HttpResponse {
    HttpResponse::Ok().json(json!({
        "status": "UP"
    }))
}