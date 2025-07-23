use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use chrono::NaiveDate;

use crate::services::GameEvaluationService;

#[derive(Deserialize)]
pub struct EvaluateDateRequest {
    pub date: NaiveDate,
}

#[derive(Serialize)]
pub struct EvaluationResponse {
    pub success: bool,
    pub message: String,
    pub games_evaluated: usize,
    pub games_updated: usize,
    pub errors: Vec<String>,
}

/// Manually trigger game evaluation for today's games
pub async fn evaluate_todays_games(
    pool: web::Data<PgPool>,
    redis: Option<web::Data<redis::Client>>,
) -> Result<HttpResponse> {
    tracing::info!("📋 Admin requested evaluation of today's games");
    
    let redis_arc = redis.map(|r| r.into_inner());
    let evaluation_service = GameEvaluationService::new_with_redis(pool.get_ref().clone(), redis_arc);
    
    match evaluation_service.evaluate_and_update_todays_games().await {
        Ok(result) => {
            let response = EvaluationResponse {
                success: result.errors.is_empty(),
                message: format!(
                    "Evaluated {} games, {} updated successfully",
                    result.games_evaluated,
                    result.games_updated
                ),
                games_evaluated: result.games_evaluated,
                games_updated: result.games_updated,
                errors: result.errors,
            };
            
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to evaluate games: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to evaluate games: {}", e)
            })))
        }
    }
}

/// Manually trigger game evaluation for a specific date
pub async fn evaluate_games_for_date(
    pool: web::Data<PgPool>,
    request: web::Json<EvaluateDateRequest>,
    redis: Option<web::Data<redis::Client>>,
) -> Result<HttpResponse> {
    tracing::info!("📋 Admin requested evaluation of games for date: {}", request.date);
    
    let redis_arc = redis.map(|r| r.into_inner());
    let evaluation_service = GameEvaluationService::new_with_redis(pool.get_ref().clone(), redis_arc);
    
    match evaluation_service.evaluate_and_update_games().await {
        Ok(result) => {
            let response = EvaluationResponse {
                success: result.errors.is_empty(),
                message: format!(
                    "Evaluated {} games for {}, {} updated successfully",
                    result.games_evaluated,
                    request.date,
                    result.games_updated
                ),
                games_evaluated: result.games_evaluated,
                games_updated: result.games_updated,
                errors: result.errors,
            };
            
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to evaluate games for date {}: {}", request.date, e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to evaluate games: {}", e)
            })))
        }
    }
}

/// Get summary of games for today
pub async fn get_todays_game_summary(
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let evaluation_service = GameEvaluationService::new(pool.get_ref().clone());
    
    match evaluation_service.get_todays_game_summary().await {
        Ok(summary) => {
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "data": {
                    "total_games": summary.total_games,
                    "scheduled_games": summary.scheduled_games,
                    "finished_games": summary.finished_games,
                    "postponed_games": summary.postponed_games,
                }
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get game summary: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to get game summary: {}", e)
            })))
        }
    }
}

/// Get summary of games for a specific date
pub async fn get_game_summary_for_date(
    pool: web::Data<PgPool>,
    request: web::Json<EvaluateDateRequest>,
) -> Result<HttpResponse> {
    let evaluation_service = GameEvaluationService::new(pool.get_ref().clone());
    
    match evaluation_service.get_games_summary_for_date(request.date).await {
        Ok(summary) => {
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "data": {
                    "date": request.date,
                    "total_games": summary.total_games,
                    "scheduled_games": summary.scheduled_games,
                    "finished_games": summary.finished_games,
                    "postponed_games": summary.postponed_games,
                }
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get game summary for {}: {}", request.date, e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": format!("Failed to get game summary: {}", e)
            })))
        }
    }
}