use actix_web::{web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;
use serde_json::json;
use chrono::Utc;
use std::sync::Arc;

use crate::middleware::auth::Claims;
use crate::models::league::*;
use crate::models::team::{TeamRegistrationRequest, TeamUpdateRequest, TeamInfo, TeamInfoWithPower};
use crate::utils::team_power;
use crate::services::player_pool_events;

/// Register a new team
#[tracing::instrument(
    name = "Register team",
    skip(team_request, pool, redis_client, claims),
    fields(
        team_name = %team_request.team_name,
        user = %claims.username
    )
)]
pub async fn register_new_team(
    team_request: web::Json<TeamRegistrationRequest>,
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<redis::Client>>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    tracing::info!("Registering team '{}' for user: {}", 
        team_request.team_name, claims.username);

    // Validate the team registration request
    if let Err(validation_error) = team_request.validate() {
        tracing::warn!("Team registration validation failed: {}", validation_error);
        return Ok(HttpResponse::BadRequest().json(json!({
            "success": false,
            "message": validation_error
        })));
    }

    // Parse user ID from claims
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Invalid user ID in claims: {}", e);
            return Ok(HttpResponse::BadRequest().json(json!({
                "success": false,
                "message": "Invalid user ID"
            })));
        }
    };

    // Check if user already has a team
    match sqlx::query!(
        "SELECT id FROM teams WHERE user_id = $1",
        user_id
    )
    .fetch_optional(pool.get_ref())
    .await
    {
        Ok(Some(_)) => {
            return Ok(HttpResponse::Conflict().json(json!({
                "success": false,
                "message": "User already has a registered team"
            })));
        }
        Ok(None) => {
            // User doesn't have a team yet, proceed with registration
        }
        Err(e) => {
            tracing::error!("Database error checking existing team: {}", e);
            return Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to check existing team registration"
            })));
        }
    }

    // Check if team name is already taken
    let sanitized_team_name = team_request.get_sanitized_name();
    match sqlx::query!(
        "SELECT id FROM teams WHERE LOWER(team_name) = LOWER($1)",
        sanitized_team_name
    )
    .fetch_optional(pool.get_ref())
    .await
    {
        Ok(Some(_)) => {
            return Ok(HttpResponse::Conflict().json(json!({
                "success": false,
                "message": "Team name already taken"
            })));
        }
        Ok(None) => {
            // Team name is available
        }
        Err(e) => {
            tracing::error!("Database error checking team name: {}", e);
            return Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to check team name availability"
            })));
        }
    }

    // Create the team
    let team_id = Uuid::new_v4();
    let now = Utc::now();

    // Start a transaction to ensure both team and membership are created atomically
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("Failed to start transaction: {}", e);
            return Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to start database transaction"
            })));
        }
    };

    // Create the team
    match sqlx::query!(
        r#"
        INSERT INTO teams (id, user_id, team_name, team_description, team_color, league_id, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
        team_id,
        user_id,
        sanitized_team_name,
        team_request.team_description,
        team_request.team_color.as_deref().unwrap_or("#4F46E5"),
        team_request.league_id,
        now,
        now
    )
    .execute(&mut *tx)
    .await
    {
        Ok(_) => {
            tracing::info!("Successfully created team '{}' with ID: {}", 
                team_request.team_name, team_id);
        }
        Err(e) => {
            tracing::error!("Failed to create team: {}", e);
            let _ = tx.rollback().await;
            return Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to create team"
            })));
        }
    }

    // Add the team owner as a member with 'owner' role
    let member_id = Uuid::new_v4();
    match sqlx::query!(
        r#"
        INSERT INTO team_members (id, team_id, user_id, role, status, joined_at, updated_at)
        VALUES ($1, $2, $3, 'owner', 'active', $4, $5)
        "#,
        member_id,
        team_id,
        user_id,
        now,
        now
    )
    .execute(&mut *tx)
    .await
    {
        Ok(_) => {
            tracing::info!("Successfully added team owner as member for team {}", team_id);
        }
        Err(e) => {
            tracing::error!("Failed to add team owner as member: {}", e);
            let _ = tx.rollback().await;
            return Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to add team owner as member"
            })));
        }
    }

    // Commit the transaction
    match tx.commit().await {
        Ok(_) => {
            tracing::info!("Successfully registered team '{}' with ID: {} and added owner as member",
                team_request.team_name, team_id);

            // Remove owner from player pool
            match sqlx::query!(
                "DELETE FROM player_pool WHERE user_id = $1",
                user_id
            )
            .execute(pool.get_ref())
            .await
            {
                Ok(_) => {
                    tracing::info!("Removed user {} from player pool after joining team", user_id);

                    // Publish player_left event (left the pool)
                    if let Err(e) = player_pool_events::publish_player_left(
                        &redis_client,
                        &pool,
                        user_id,
                        claims.username.clone(),
                        None, // league_id
                    ).await {
                        tracing::warn!("Failed to publish player_left event: {}", e);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to remove user from player pool: {}", e);
                    // Don't fail team registration if pool removal fails
                }
            }

            // Publish player_assigned event for team owner (non-blocking)
            if let Err(e) = player_pool_events::publish_player_assigned(
                &redis_client,
                &pool,
                user_id,
                claims.username.clone(),
                None, // league_id
                team_id,
                team_request.team_name.clone(),
            ).await {
                tracing::warn!("Failed to publish player_assigned event for team owner: {}", e);
                // Don't fail team registration if notification fails
            }

            Ok(HttpResponse::Created().json(json!({
                "success": true,
                "message": "Team registered successfully",
                "data": {
                    "team_id": team_id,
                    "team_name": team_request.team_name,
                    "user_id": user_id,
                    "created_at": now
                }
            })))
        }
        Err(e) => {
            tracing::error!("Failed to commit team registration transaction: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to complete team registration"
            })))
        }
    }
}

/// Get team information
pub async fn get_team_information(
    team_id: Uuid,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    // First get the basic team info
    let team = match sqlx::query_as!(
        TeamInfo,
        r#"
        SELECT 
            t.id,
            t.user_id,
            t.team_name,
            t.team_description,
            t.team_color,
            t.league_id,
            t.created_at,
            t.updated_at,
            u.username as owner_username
        FROM teams t
        JOIN users u ON t.user_id = u.id
        WHERE t.id = $1
        "#,
        team_id
    )
    .fetch_optional(pool.get_ref())
    .await
    {
        Ok(Some(team)) => team,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(json!({
                "success": false,
                "message": "Team not found"
            })));
        }
        Err(e) => {
            tracing::error!("Failed to get team {}: {}", team_id, e);
            return Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to retrieve team information"
            })));
        }
    };

    // Calculate team power
    let team_power = match team_power::calculate_team_power(team_id, pool.get_ref()).await {
        Ok(power) => power,
        Err(e) => {
            tracing::error!("Failed to calculate team power for {}: {}", team_id, e);
            0.0 // Default to 0 if calculation fails
        }
    };

    // Create TeamInfoWithPower
    let team_with_power = TeamInfoWithPower {
        id: team.id,
        user_id: team.user_id,
        team_name: team.team_name,
        team_description: team.team_description,
        team_color: team.team_color,
        league_id: team.league_id,
        created_at: team.created_at,
        updated_at: team.updated_at,
        owner_username: team.owner_username,
        total_power: team_power,
    };

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "data": team_with_power
    })))
}

/// Get all registered teams
pub async fn get_all_registered_teams(
    query: web::Query<PaginationQuery>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let limit = query.limit.unwrap_or(20).min(100);
    
    // First get the basic team info
    let teams = match sqlx::query_as!(
        TeamInfo,
        r#"
        SELECT 
            t.id,
            t.user_id,
            t.team_name,
            t.team_description,
            t.team_color,
            t.league_id,
            t.created_at,
            t.updated_at,
            u.username as owner_username
        FROM teams t
        JOIN users u ON t.user_id = u.id
        ORDER BY t.created_at DESC
        LIMIT $1
        "#,
        limit as i64
    )
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(teams) => teams,
        Err(e) => {
            tracing::error!("Failed to get teams: {}", e);
            return Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to retrieve teams"
            })));
        }
    };

    // Calculate power for all teams
    let team_ids: Vec<Uuid> = teams.iter().map(|t| t.id).collect();
    let team_powers = match team_power::calculate_multiple_team_powers(&team_ids, pool.get_ref()).await {
        Ok(powers) => powers,
        Err(e) => {
            tracing::error!("Failed to calculate team powers: {}", e);
            return Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to calculate team powers"
            })));
        }
    };

    // Convert to TeamInfoWithPower
    let teams_with_power: Vec<TeamInfoWithPower> = teams
        .into_iter()
        .map(|team| TeamInfoWithPower {
            total_power: team_powers.get(&team.id).copied().unwrap_or(0.0),
            id: team.id,
            user_id: team.user_id,
            team_name: team.team_name,
            team_description: team.team_description,
            team_color: team.team_color,
            league_id: team.league_id,
            created_at: team.created_at,
            updated_at: team.updated_at,
            owner_username: team.owner_username,
        })
        .collect();

    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "data": teams_with_power,
        "pagination": {
            "limit": limit,
            "total": teams_with_power.len()
        }
    })))
}

/// Update team information
pub async fn update_team_information(
    team_id: Uuid,
    team_update: web::Json<TeamUpdateRequest>,
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    // Validate the update request
    if let Err(validation_error) = team_update.validate() {
        tracing::warn!("Team update validation failed: {}", validation_error);
        return Ok(HttpResponse::BadRequest().json(json!({
            "success": false,
            "message": validation_error
        })));
    }
    
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            tracing::error!("Invalid user ID in claims: {}", e);
            return Ok(HttpResponse::BadRequest().json(json!({
                "success": false,
                "message": "Invalid user ID"
            })));
        }
    };

    // Verify user owns this team
    match sqlx::query!(
        "SELECT user_id FROM teams WHERE id = $1",
        team_id
    )
    .fetch_optional(pool.get_ref())
    .await
    {
        Ok(Some(team)) => {
            if team.user_id != user_id {
                return Ok(HttpResponse::Forbidden().json(json!({
                    "success": false,
                    "message": "You can only update your own team"
                })));
            }
        }
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(json!({
                "success": false,
                "message": "Team not found"
            })));
        }
        Err(e) => {
            tracing::error!("Database error checking team ownership: {}", e);
            return Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to verify team ownership"
            })));
        }
    }

    // Update team information
    match sqlx::query!(
        r#"
        UPDATE teams 
        SET team_name = COALESCE($1, team_name),
            team_description = COALESCE($2, team_description),
            team_color = COALESCE($3, team_color),
            updated_at = NOW()
        WHERE id = $4
        "#,
        team_update.team_name.as_deref(),
        team_update.team_description.as_deref(),
        team_update.team_color.as_deref(),
        team_id
    )
    .execute(pool.get_ref())
    .await
    {
        Ok(_) => {
            tracing::info!("Successfully updated team {}", team_id);
            Ok(HttpResponse::Ok().json(json!({
                "success": true,
                "message": "Team updated successfully"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to update team {}: {}", team_id, e);
            Ok(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Failed to update team"
            })))
        }
    }
}

/// Get team league history
pub async fn get_team_league_history(
    _team_id: Uuid,
    _pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    // Implementation for team history
    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "data": [],
        "message": "Team history endpoint - implementation needed"
    })))
}