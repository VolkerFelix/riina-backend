use actix_web::{web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;
use serde_json::json;

use crate::league::league::LeagueService;
use crate::middleware::auth::Claims;
use crate::models::league::*;

/// Create a new league season
#[tracing::instrument(
    name = "Create league season",
    skip(season_request, pool, claims),
    fields(
        season_name = %season_request.name,
        team_count = %season_request.team_ids.len(),
        admin_user = %claims.username
    )
)]
pub async fn create_league_season(
    season_request: web::Json<CreateSeasonRequest>,
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    tracing::info!("Creating league season '{}' with {} teams by admin: {}", 
        season_request.name, season_request.team_ids.len(), claims.username);

    let league_service = LeagueService::new(pool.get_ref().clone());
    
    match league_service.create_season(season_request.into_inner()).await {
        Ok(schedule_response) => {
            tracing::info!("Successfully created league season with {} games", 
                schedule_response.games.len());
            
            Ok(HttpResponse::Created().json(json!({
                "success": true,
                "message": "League season created successfully",
                "data": schedule_response
            })))
        }
        Err(e) => {
            tracing::error!("Failed to create league season: {}", e);
            Ok(HttpResponse::BadRequest().json(json!({
                "success": false,
                "message": format!("Failed to create league season: {}", e)
            })))
        }
    }
}

/// Get recent results
pub async fn get_league_recent_results(
    _query: web::Query<RecentResultsQuery>,
    _pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    // Implementation for recent results
    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "data": [],
        "message": "Recent results endpoint - implementation needed"
    })))
}

/// Get game week
pub async fn get_league_game_week(
    _season_id: Uuid,
    _week_number: i32,
    _pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    // Implementation for game week
    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "data": {},
        "message": "Game week endpoint - implementation needed"
    })))
}