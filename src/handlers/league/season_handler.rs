use actix_web::{web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;
use serde_json::json;
use crate::league::league::LeagueService;
use crate::models::league::{LeagueSeason, PaginationQuery};

/// Get active league season
pub async fn get_active_league_season(
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let league_service = LeagueService::new(pool.get_ref().clone());
    
    match league_service.get_active_seasons().await {
        Ok(seasons) => {
            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "data": seasons
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get active seasons: {}", e);
            Ok(HttpResponse::NotFound().json(json!({
                "success": false,
                "message": "No active seasons found"
            })))
        }
    }
}

/// Get specific league season
pub async fn get_league_season(
    season_id: Uuid,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let league_service = LeagueService::new(pool.get_ref().clone());
    
    match league_service.get_schedule(season_id).await {
        Ok(schedule) => {
            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "data": schedule
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get season {}: {}", season_id, e);
            Ok(HttpResponse::NotFound().json(json!({
                "success": false,
                "message": "Season not found"
            })))
        }
    }
}

/// Get all league seasons with pagination
pub async fn get_all_league_seasons(
    query: web::Query<PaginationQuery>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let limit = query.limit.unwrap_or(10).min(50);
    
    match sqlx::query_as!(
        LeagueSeason,
        "SELECT * FROM league_seasons ORDER BY created_at DESC LIMIT $1",
        limit as i64
    )
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(seasons) => {
            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "data": seasons,
                "pagination": {
                    "limit": limit,
                    "total": seasons.len()
                }
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get seasons: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to retrieve seasons"
            })))
        }
    }
}

/// Get league schedule
pub async fn get_league_schedule(
    season_id: Uuid,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let league_service = LeagueService::new(pool.get_ref().clone());
    
    match league_service.get_schedule(season_id).await {
        Ok(schedule) => {
            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "data": schedule
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get schedule for season {}: {}", season_id, e);
            Ok(HttpResponse::NotFound().json(json!({
                "success": false,
                "message": "Schedule not found"
            })))
        }
    }
}

/// Get league standings
pub async fn get_league_standings(
    season_id: Uuid,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let league_service = LeagueService::new(pool.get_ref().clone());
    
    match league_service.get_standings(season_id).await {
        Ok(standings) => {
            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "data": standings
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get standings for season {}: {}", season_id, e);
            Ok(HttpResponse::NotFound().json(json!({
                "success": false,
                "message": "Standings not found"
            })))
        }
    }
}