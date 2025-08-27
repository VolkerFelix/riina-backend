// Enhanced src/handlers/workout_data/upload_health_data.rs - Now with game stats!

use actix_web::{web, HttpResponse};
use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;
use redis::AsyncCommands;
use std::sync::Arc;
use crate::middleware::auth::Claims;
use crate::db::workout_data::{insert_workout_data, check_duplicate_workout_by_time};
use crate::models::workout_data::{WorkoutDataSyncRequest, WorkoutUploadResponse, GameStats, StatChanges as WorkoutStatChanges};
use crate::models::common::ApiResponse;
use crate::game::stats_calculator::StatCalculator;
use crate::models::league::{LiveGameScoreUpdate, LeagueGame};
use crate::db::game_queries::GameQueries;
use crate::game::stats_calculator::StatChanges;
use crate::models::game_events::GameEvent;

#[tracing::instrument(
    name = "Upload workout data with game stats",
    skip(data, pool, redis, claims),
    fields(
        username = %claims.username,
        data_type = %data.device_id
    )
)]
pub async fn upload_workout_data(
    data: web::Json<WorkoutDataSyncRequest>,
    pool: web::Data<sqlx::PgPool>,
    redis: Option<web::Data<Arc<redis::Client>>>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    tracing::info!("🎮 Processing workout data with game mechanics for user: {}", claims.username);
    
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => {
            tracing::info!("User ID parsed successfully: {}", id);
            id
        },
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Invalid user ID")
            );
        }
    };

    // workout_uuid is now required - database constraint will prevent duplicates
    tracing::info!("🔍 Processing workout UUID: {}", data.workout_uuid);

    // Check for duplicate workouts based on time (with 15-second tolerance)
    let is_duplicate = match check_duplicate_workout_by_time(&pool, user_id, &data).await {
        Ok(is_dup) => {
            if is_dup {
                tracing::warn!("⚠️ Duplicate workout detected based on time overlap for user: {}. Rejecting duplicate.", claims.username);
                let response = WorkoutUploadResponse {
                    sync_id: Uuid::new_v4(),
                    timestamp: Utc::now(),
                    is_duplicate: true,
                    duplicate_reason: Some("Similar workout time detected (within 15 seconds)".to_string()),
                    action: Some("rejected".to_string()),
                    game_stats: None,
                };
                return HttpResponse::Ok().json(ApiResponse::success(
                    "Workout already exists (duplicate detected)",
                    response
                ));
            }
            is_dup
        }
        Err(e) => {
            tracing::error!("❌ Failed to check for duplicate workouts: {}", e);
            // Continue anyway - assume not duplicate
            false
        }
    };

    // 🎲 CALCULATE GAME STATS FROM WORKOUT DATA (only non-duplicates reach here)
    let stat_changes = StatCalculator::calculate_stat_changes(&pool, user_id, &data).await;
    tracing::info!("📊 Calculated stat changes for {}: +{} stamina, +{} strength", 
        claims.username, 
        stat_changes.stamina_change, 
        stat_changes.strength_change, 
    );

    // 💾 APPLY STAT CHANGES TO DATABASE
    let update_result = sqlx::query!(
        r#"
        UPDATE user_avatars 
        SET stamina = stamina + $1, 
            strength = strength + $2
        WHERE user_id = $3
        "#,
        stat_changes.stamina_change,
        stat_changes.strength_change,
        user_id
    )
    .execute(&**pool)
    .await;

    match update_result {
        Ok(_) => {
            tracing::info!("✅ Successfully updated avatar stats for {}", claims.username);
        }
        Err(e) => {
            tracing::error!("❌ Failed to update avatar stats for {}: {}", claims.username, e);
            return HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error("Failed to update avatar stats")
            );
        }
    }

    // Insert workout data into database (only non-duplicates)
    tracing::info!("💾 Inserting workout data into database for user: {} with workout_uuid: {:?}", 
        claims.username, data.workout_uuid);
    let insert_result = insert_workout_data(&pool, user_id, &data).await;
    
    match insert_result {
        Ok(sync_id) => {
            tracing::info!("✅ Workout data inserted successfully with sync_id: {} for user: {}", 
                sync_id, claims.username);
                
            // Only update stats and live games if NOT a duplicate
            if !is_duplicate {
                // 📊 UPDATE WORKOUT DATA WITH CALCULATED STATS
                let zone_breakdown_json = stat_changes.zone_breakdown.as_ref()
                    .map(|breakdown| serde_json::to_value(breakdown).unwrap_or(serde_json::Value::Null));

                let update_result = sqlx::query!(
                    r#"
                    UPDATE workout_data 
                    SET heart_rate_zones = $1,
                        stamina_gained = $2,
                        strength_gained = $3,
                        total_points_gained = $4
                    WHERE id = $5
                    "#,
                    zone_breakdown_json,
                    stat_changes.stamina_change,
                    stat_changes.strength_change,
                    stat_changes.stamina_change + stat_changes.strength_change,
                    sync_id
                )
                .execute(&**pool)
                .await;

                if let Err(e) = update_result {
                    tracing::error!("❌ Failed to update workout data with calculated stats for workout {}: {}", sync_id, e);
                } else {
                    tracing::info!("✅ Successfully updated workout data with zone breakdown and stat gains for workout {}", sync_id);
                }

                // 🏆 CHECK FOR ACTIVE GAMES AND UPDATE SCORES
                if let (Some(workout_start), Some(workout_end)) = (data.workout_start, data.workout_end) {
                    match check_and_update_active_games(
                        user_id, 
                        &claims.username,
                        sync_id,
                        &stat_changes,
                        &workout_start,
                        &workout_end,
                        &pool,
                    ).await {
                        Ok(_) => {
                            tracing::info!("✅ Successfully updated game scores for user {}", claims.username);
                        }
                        Err(e) => {
                            tracing::error!("❌ Failed to update game scores for user {}: {}", claims.username, e);
                        }
                    }
                } else {
                    tracing::warn!("⚠️ Workout start or end time missing for user {} - cannot score in live games", claims.username);
                }
            } else {
                tracing::info!("⏭️ Skipping stats update and live game scoring for duplicate workout");
            }
            // 🎯 PREPARE GAME EVENT FOR REAL-TIME NOTIFICATION
            let game_event = json!({
                "event_type": "workout_data_processed",
                "user_id": user_id.to_string(),
                "username": claims.username,
                "sync_id": sync_id.to_string(),
                "stat_changes": {
                    "stamina_change": stat_changes.stamina_change,
                    "strength_change": stat_changes.strength_change,
                },
                "reasoning": stat_changes.reasoning,
                "timestamp": Utc::now().to_rfc3339()
            });

            // 📡 PUBLISH TO REDIS FOR REAL-TIME NOTIFICATION
            if let Some(redis_client) = &redis {
                let user_channel = format!("game:events:user:{}", user_id);
                let global_channel = "game:events:global".to_string();
                let event_str = serde_json::to_string(&game_event)
                    .unwrap_or_else(|e| {
                        tracing::error!("Failed to serialize game event: {}", e);
                        "{}".to_string()
                    });

                let redis_client = redis_client.clone();
                let event_str_clone = event_str.clone();
                let username_clone = claims.username.clone();
                
                tokio::spawn(async move {
                    match redis_client.get_async_connection().await {
                        Ok(mut conn) => {
                            // Publish to user-specific channel
                            let user_result: Result<i32, redis::RedisError> = 
                                conn.publish(&user_channel, &event_str).await;
                            
                            // Also publish to global channel for leaderboards/social features
                            let global_result: Result<i32, redis::RedisError> = 
                                conn.publish(&global_channel, &event_str_clone).await;
                            
                            match (user_result, global_result) {
                                (Ok(user_receivers), Ok(global_receivers)) => {
                                    tracing::info!("🎮 Published game event for {} to {} user subscribers and {} global subscribers", 
                                        username_clone, user_receivers, global_receivers);
                                }
                                (Err(e), _) | (_, Err(e)) => {
                                    tracing::error!("❌ Failed to publish game event for {}: {}", username_clone, e);
                                }
                            }
                        },
                        Err(e) => {
                            tracing::error!("❌ Redis connection failed during game event publishing: {}", e);
                        }
                    }
                });
            } else {
                tracing::warn!("⚠️  Redis not available - game events will not be published in real-time");
            }

            // 🎉 RESPONSE WITH GAME STATS (only non-duplicates reach here)
            let message = "Workout data synced and game stats calculated!";
            let response = WorkoutUploadResponse {
                sync_id,
                timestamp: Utc::now(),
                is_duplicate: false,
                duplicate_reason: None,
                action: None,
                game_stats: Some(GameStats {
                    stat_changes: WorkoutStatChanges {
                        stamina_change: stat_changes.stamina_change,
                        strength_change: stat_changes.strength_change,
                    },
                    reasoning: stat_changes.reasoning.join("\n"),
                    summary: format!("Gained {} total stat points!", 
                        stat_changes.stamina_change + stat_changes.strength_change
                    ),
                }),
            };

            tracing::info!("✅ Workout data processed successfully for {}: {}", 
                claims.username, sync_id);
            HttpResponse::Ok().json(
                ApiResponse::success(message, response)
            )
        }
        Err(e) => {
            // Check if this is a duplicate workout UUID error
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.code().as_deref() == Some("23505") {
                    tracing::error!("❌ DUPLICATE WORKOUT UUID: Failed to sync workout data for {} due to duplicate workout_uuid: {:?}. This workout has already been uploaded.", 
                        claims.username, data.workout_uuid);
                    
                    // Return a more specific error response for duplicate UUIDs
                    return HttpResponse::Conflict().json(
                        ApiResponse::<()>::error("This workout has already been uploaded (duplicate UUID)")
                    );
                }
            }
            
            tracing::error!("❌ Failed to sync workout data for {}: {}", claims.username, e);
            HttpResponse::InternalServerError().json(
                ApiResponse::<()>::error(format!("Failed to sync workout data: {}", e))
            )
        }
    }
}

/// Check if user is in any active games and update scores using consolidated architecture
async fn check_and_update_active_games(
    user_id: Uuid,
    username: &str,
    workout_data_id: Uuid,
    stat_changes: &StatChanges,
    workout_start_time: &DateTime<Utc>,
    workout_end_time: &DateTime<Utc>,
    pool: &sqlx::PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🎮 Checking for active games for user {}", username);

    let game_queries = GameQueries::new(pool.clone());
    let active_games = game_queries.get_active_games().await?;
    
    if active_games.is_empty() {
        tracing::debug!("No active games found for user {}", username);
        return Ok(());
    }

    tracing::info!("🏆 Found {} active game(s) to check for user {}", active_games.len(), username);

    for game in active_games {
        // Check if user is a member of either team in this game
        let user_team_id = match get_user_team_for_game(user_id, &game, pool).await {
            Ok(team_id) => team_id,
            Err(_) => {
                tracing::debug!("User {} is not a member of teams playing in game {}", username, game.id);
                continue;
            }
        };

        // Check if the workout time falls within the game's live scoring period
        if let (Some(game_start), Some(game_end)) = (game.game_start_time, game.game_end_time) {
            if workout_start_time >= &game_start && workout_end_time <= &game_end {
                tracing::info!("🏆 Workout time is within live game period for user {} in game {} ({} to {})", 
                              username, game.id, workout_start_time, workout_end_time);
                update_game_score_from_workout(
                    user_id,
                    username,
                    user_team_id,
                    &game,
                    stat_changes,
                    workout_data_id,
                    pool,
                ).await?;
            } else {
                tracing::debug!("❌ Workout time ({} to {}) is outside live game period ({} to {}) for user {} in game {}", 
                               workout_start_time, workout_end_time, game_start, game_end, username, game.id);
            }
        } else {
            tracing::debug!("❌ Game {} does not have live scoring times set", game.id);
        }
    }

    Ok(())
}

/// Update game score based on workout stats using consolidated games table
async fn update_game_score_from_workout(
    user_id: Uuid,
    username: &str,
    user_team_id: Uuid,
    game: &LeagueGame,
    stat_changes: &StatChanges,
    workout_data_id: Uuid,
    pool: &sqlx::PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🏆 Updating game score for user {} in game {}", username, game.id);

    // Simple scoring: just add up stamina and strength gains
    let score_increase = stat_changes.stamina_change + stat_changes.strength_change;
    
    tracing::info!("📊 Score calculation for {}: stamina={}, strength={}, score_increase={}", 
        username, stat_changes.stamina_change, stat_changes.strength_change, score_increase);

    // Create score update object
    let score_update = LiveGameScoreUpdate {
        user_id,
        username: username.to_string(),
        score_increase,
    };

    // Update the game score using GameQueries
    let game_queries = GameQueries::new(pool.clone());
    game_queries.update_game_score(game.id, &score_update).await?;

    // Determine which team side (home or away)
    let team_side = if user_team_id == game.home_team_id {
        "home"
    } else {
        "away"
    };

    // Record the scoring event with all required fields
    record_score_event(
        game.id, 
        user_id, 
        username,
        user_team_id,
        team_side,
        score_increase, 
        stat_changes.stamina_change,
        stat_changes.strength_change,
        workout_data_id,
        pool
    ).await?;

    // Broadcast score update via WebSocket
    broadcast_score_update(game.id, pool).await.unwrap_or_else(|e| {
        tracing::error!("Failed to broadcast score update: {}", e);
    });

    tracing::info!("✅ Successfully updated score for game {} by {} points from user {}", 
        game.id, score_increase, username);

    Ok(())
}

/// Helper function to determine which team a user belongs to in a game
async fn get_user_team_for_game(
    user_id: Uuid, 
    game: &LeagueGame,
    pool: &sqlx::PgPool
) -> Result<Uuid, Box<dyn std::error::Error>> {
    let membership = sqlx::query!(
        r#"
        SELECT team_id 
        FROM team_members 
        WHERE user_id = $1 
        AND status = 'active'
        AND (team_id = $2 OR team_id = $3)
        "#,
        user_id,
        game.home_team_id,
        game.away_team_id
    )
    .fetch_optional(pool)
    .await?;
    
    match membership {
        Some(m) => Ok(m.team_id),
        None => Err("User does not belong to either team in this game".into())
    }
}

/// Record a scoring event in the live_score_events table
async fn record_score_event(
    game_id: Uuid,
    user_id: Uuid,
    username: &str,
    team_id: Uuid,
    team_side: &str,
    score_increase: i32,
    stamina_gained: i32,
    strength_gained: i32,
    workout_data_id: Uuid,
    pool: &sqlx::PgPool,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO live_score_events (
            id, game_id, user_id, username, team_id, team_side,
            score_points, power_contribution, stamina_gained, strength_gained,
            event_type, description, workout_data_id, occurred_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'workout_upload', $11, $12, NOW())
        "#,
        Uuid::new_v4(),
        game_id,
        user_id,
        username,
        team_id,
        team_side,
        score_increase, // score_points
        0i32, // power_contribution (no longer used, set to 0)
        stamina_gained,
        strength_gained,
        format!("Workout completed: +{} stamina, +{} strength", stamina_gained, strength_gained),
        workout_data_id
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Broadcast game score update via WebSocket
async fn broadcast_score_update(
    game_id: Uuid,
    pool: &sqlx::PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get updated game information with team names
    let game_data = sqlx::query!(
        r#"
        SELECT 
            g.id, g.home_team_id, g.away_team_id, 
            g.home_score, g.away_score, g.status,
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
    .fetch_optional(pool)
    .await?;

    if let Some(game) = game_data {
        // Calculate game progress (simplified)
        let game_progress = if let (Some(start), Some(end)) = (game.game_start_time, game.game_end_time) {
            let now = chrono::Utc::now();
            let total_duration = end - start;
            let elapsed = now - start;
            
            if elapsed.num_seconds() < 0 {
                0.0
            } else if elapsed > total_duration {
                100.0
            } else {
                (elapsed.num_seconds() as f32 / total_duration.num_seconds() as f32) * 100.0
            }
        } else {
            0.0
        };

        let game_event = GameEvent::LiveScoreUpdate {
            game_id: game.id,
            home_team_id: game.home_team_id,
            home_team_name: game.home_team_name,
            away_team_id: game.away_team_id,
            away_team_name: game.away_team_name,
            home_score: game.home_score as u32,
            away_score: game.away_score as u32,
            game_progress,
            game_time_remaining: None, // TODO: Calculate remaining time
            is_active: game.status == "in_progress",
            last_updated: chrono::Utc::now(),
        };

        // TODO: Implement actual WebSocket broadcasting via Redis
        if let GameEvent::LiveScoreUpdate { home_team_name, home_score, away_score, away_team_name, .. } = &game_event {
            tracing::info!("Broadcasting score update for game {}: {} {} - {} {}", 
                game.id, home_team_name, home_score, away_score, away_team_name);
        }
    }

    Ok(())
}

