use actix_web::{web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;
use serde::Serialize;
use std::sync::Arc;

use crate::services::ManageGameService;
use crate::middleware::auth::Claims;
use crate::models::league::PaginationQuery;
// Removed unused import: use crate::db::game_queries::GameQueries;

#[derive(Serialize)]
pub struct LiveGameScore {
    pub game_id: Uuid,
    pub home_team_name: String,
    pub away_team_name: String,
    pub home_score: u32,
    pub away_score: u32,
    pub week_number: i32,
    pub status: String,
}

#[derive(Serialize)]
pub struct LiveScoresResponse {
    pub success: bool,
    pub data: Vec<LiveGameScore>,
    pub total_active_games: usize,
}

#[derive(Serialize)]
pub struct GameManagementResponse {
    pub success: bool,
    pub started_games: Vec<Uuid>,
    pub finished_games: Vec<Uuid>,
    pub message: String,
}

/// Get live scores for all currently active games - now just returns active games without scores
pub async fn get_live_scores(
    pool: web::Data<PgPool>,
    _redis_client: web::Data<Arc<redis::Client>>,
    _claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let week_game_service = ManageGameService::new(pool.get_ref().clone());
    
    match week_game_service.get_active_games().await {
        Ok(games) => {
            let game_list: Vec<LiveGameScore> = games
                .into_iter()
                .map(|game| LiveGameScore {
                    game_id: game.id,
                    home_team_name: "TBD".to_string(), // Team names would need separate query
                    away_team_name: "TBD".to_string(),
                    home_score: 0, // No live scoring, just show game is active
                    away_score: 0,
                    week_number: game.week_number,
                    status: game.status.as_str().to_string(),
                })
                .collect();

            let response = LiveScoresResponse {
                success: true,
                total_active_games: game_list.len(),
                data: game_list,
            };

            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to get active games: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Failed to get active games"
            })))
        }
    }
}

/// Get specific game details with actual live scoring data
pub async fn get_game_live_score(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    query: web::Query<PaginationQuery>,
    _claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let game_id = path.into_inner();
    
    // Set pagination defaults
    let page = query.page.unwrap_or(1).max(1);
    let limit = query.limit.unwrap_or(10).clamp(1, 100);
    let offset = (page - 1) * limit;
    
    // Get game info with live scoring data from unified games table
    let game = sqlx::query!(
        r#"
        SELECT 
            g.id, g.week_number, g.status,
            g.home_score, g.away_score,
            g.game_start_time, g.game_end_time,
            ht.team_name as home_team_name,
            at.team_name as away_team_name
        FROM games g
        JOIN teams ht ON g.home_team_id = ht.id
        JOIN teams at ON g.away_team_id = at.id
        WHERE g.id = $1
        "#,
        game_id
    )
    .fetch_optional(pool.get_ref())
    .await;

    match game {
        Ok(Some(game_data)) => {
            let home_score = game_data.home_score as u32;
            let away_score = game_data.away_score as u32;
            
            // Get total count of scoring events for pagination
            let total_count = sqlx::query!(
                r#"
                SELECT COUNT(*) as "count!"
                FROM live_score_events
                WHERE game_id = $1
                "#,
                game_id
            )
            .fetch_one(pool.get_ref())
            .await
            .map(|r| r.count)
            .unwrap_or(0);
            
            // Fetch scoring events from live_score_events table with workout details
            let scoring_events = sqlx::query!(
                r#"
                SELECT
                    lse.id, lse.user_id, lse.score_points,
                    lse.occurred_at, lse.event_type::text as "event_type!", lse.description,
                    lse.username, lse.team_id, lse.team_side, lse.workout_data_id,
                    lse.stamina_gained, lse.strength_gained,
                    u.profile_picture_url as "profile_picture_url?",
                    wd.id as "workout_id?", wd.created_at as "workout_date?",
                    wd.workout_start as "workout_start?", wd.workout_end as "workout_end?",
                    wd.activity_name as "activity_name?", wd.user_activity as "user_activity?",
                    wd.avg_heart_rate as "avg_heart_rate?", wd.max_heart_rate as "max_heart_rate?",
                    wd.duration_minutes as "duration_minutes?",
                    wd.heart_rate_zones as "heart_rate_zones?",
                    p.media_urls as "media_urls?",
                    p.content as "post_content?",
                    wsf.effort_rating as "effort_rating?"
                FROM live_score_events lse
                LEFT JOIN users u ON u.id = lse.user_id
                LEFT JOIN workout_data wd ON wd.id = lse.workout_data_id
                LEFT JOIN posts p ON p.workout_id = wd.id
                LEFT JOIN workout_scoring_feedback wsf ON wsf.workout_data_id = wd.id AND wsf.user_id = lse.user_id
                WHERE lse.game_id = $1
                ORDER BY lse.occurred_at DESC
                LIMIT $2 OFFSET $3
                "#,
                game_id,
                limit,
                offset
            )
            .fetch_all(pool.get_ref())
            .await
            .unwrap_or_else(|_| vec![]);

            let scoring_events_json: Vec<serde_json::Value> = scoring_events
                .into_iter()
                .map(|event| {
                    let mut event_json = serde_json::json!({
                        "id": event.id,
                        "user_id": event.user_id,
                        "username": event.username,
                        "profile_picture_url": event.profile_picture_url,
                        "team_id": event.team_id,
                        "team_side": event.team_side,
                        "score_points": event.score_points,
                        "occurred_at": event.occurred_at,
                        "event_type": event.event_type.to_string(),
                        "description": event.description
                    });
                    
                    // Add workout details if available
                    if event.workout_id.is_some() {
                        event_json["workout_details"] = serde_json::json!({
                            "id": event.workout_id,
                            "workout_date": event.workout_date,
                            "workout_start": event.workout_start,
                            "workout_end": event.workout_end,
                            "activity_name": event.activity_name,
                            "user_activity": event.user_activity,
                            "stamina_gained": event.stamina_gained,
                            "strength_gained": event.strength_gained,
                            "avg_heart_rate": event.avg_heart_rate,
                            "max_heart_rate": event.max_heart_rate,
                            "duration_minutes": event.duration_minutes,
                            "heart_rate_zones": event.heart_rate_zones,
                            "media_urls": event.media_urls,
                            "post_content": event.post_content,
                            "effort_rating": event.effort_rating,
                            "needs_effort_rating": event.effort_rating.is_none()
                        });
                    }
                    
                    event_json
                })
                .collect();

            let total_pages = ((total_count as f64) / (limit as f64)).ceil() as i64;
            
            let mut game_info = serde_json::json!({
                "game_id": game_id,
                "home_team_name": game_data.home_team_name,
                "away_team_name": game_data.away_team_name,
                "home_score": home_score,
                "away_score": away_score,
                "week_number": game_data.week_number,
                "status": game_data.status,
                "scoring_events": scoring_events_json,
                "pagination": {
                    "page": page,
                    "limit": limit,
                    "total_count": total_count,
                    "total_pages": total_pages,
                    "has_next": page < total_pages,
                    "has_prev": page > 1
                }
            });

            // Add optional game timing fields
            if let Some(start_time) = game_data.game_start_time {
                game_info["game_start_time"] = serde_json::Value::String(start_time.to_rfc3339());
            }
            if let Some(end_time) = game_data.game_end_time {
                game_info["game_end_time"] = serde_json::Value::String(end_time.to_rfc3339());
            }

            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "data": game_info
            })))
        }
        Ok(None) => {
            Ok(HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "error": "Game not found"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get game details for {}: {}", game_id, e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Failed to get game details"
            })))
        }
    }
}

/// Admin endpoint to manually trigger game management cycle
pub async fn manage_games(
    pool: web::Data<PgPool>,
    _redis_client: web::Data<Arc<redis::Client>>,
) -> Result<HttpResponse> {
    let week_game_service = ManageGameService::new(pool.get_ref().clone());
    
    match week_game_service.run_game_cycle().await {
        Ok((games_ready_to_start, live_games, started_games, finished_games)) => {
            let message = format!(
                "Ready to start {} games, live {} games, started {} games, finished {} games", 
                games_ready_to_start.len(), 
                live_games.len(), 
                started_games.len(), 
                finished_games.len()
            );
            
            let response = GameManagementResponse {
                success: true,
                started_games,
                finished_games,
                message,
            };

            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => {
            tracing::error!("Failed to manage games: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Failed to manage games"
            })))
        }
    }
}

/// Get all currently active games
pub async fn get_active_games(
    pool: web::Data<PgPool>,
    _redis_client: web::Data<Arc<redis::Client>>,
    _claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let week_game_service = ManageGameService::new(pool.get_ref().clone());

    match week_game_service.get_active_games().await {
        Ok(games) => {
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "data": games,
                "total_count": games.len()
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get active games: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Failed to get active games"
            })))
        }
    }
}

#[derive(Serialize)]
pub struct PlayerScore {
    pub user_id: Uuid,
    pub username: String,
    pub profile_picture_url: Option<String>,
    pub team_id: Uuid,
    pub team_name: String,
    pub team_side: String,
    pub total_points: i32,
    pub event_count: i64,
}

/// GET /league/games/{game_id}/player-scores - Get aggregated player scores for a game
/// Returns total points scored by each player in the game, calculated directly in the database
pub async fn get_game_player_scores(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    _claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let game_id = path.into_inner();

    // Get game info to verify it exists and get team names
    let game = sqlx::query!(
        r#"
        SELECT
            g.id,
            ht.id as home_team_id,
            ht.team_name as home_team_name,
            at.id as away_team_id,
            at.team_name as away_team_name
        FROM games g
        JOIN teams ht ON g.home_team_id = ht.id
        JOIN teams at ON g.away_team_id = at.id
        WHERE g.id = $1
        "#,
        game_id
    )
    .fetch_optional(pool.get_ref())
    .await;

    match game {
        Ok(Some(game_data)) => {
            // Aggregate player scores directly in the database for efficiency
            let player_scores = sqlx::query!(
                r#"
                SELECT
                    lse.user_id,
                    lse.username,
                    u.profile_picture_url,
                    lse.team_id,
                    lse.team_side,
                    SUM(lse.score_points) as "total_points!",
                    COUNT(*) as "event_count!"
                FROM live_score_events lse
                LEFT JOIN users u ON u.id = lse.user_id
                WHERE lse.game_id = $1
                GROUP BY lse.user_id, lse.username, u.profile_picture_url, lse.team_id, lse.team_side
                ORDER BY SUM(lse.score_points) DESC
                "#,
                game_id
            )
            .fetch_all(pool.get_ref())
            .await
            .unwrap_or_else(|_| vec![]);

            let scores: Vec<PlayerScore> = player_scores
                .into_iter()
                .map(|row| {
                    let team_name = if row.team_side == "home" {
                        game_data.home_team_name.clone()
                    } else {
                        game_data.away_team_name.clone()
                    };

                    PlayerScore {
                        user_id: row.user_id,
                        username: row.username,
                        profile_picture_url: row.profile_picture_url,
                        team_id: row.team_id,
                        team_name,
                        team_side: row.team_side,
                        total_points: row.total_points as i32,
                        event_count: row.event_count,
                    }
                })
                .collect();

            Ok(HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "data": {
                    "game_id": game_id,
                    "player_scores": scores,
                    "total_players": scores.len()
                }
            })))
        }
        Ok(None) => {
            Ok(HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "error": "Game not found"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to get game player scores for {}: {}", game_id, e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Failed to get player scores"
            })))
        }
    }
}