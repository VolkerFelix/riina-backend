// Enhanced src/handlers/workout_data/upload_health_data.rs - Now with game stats!

use actix_web::{web, HttpResponse};
use chrono::{DateTime, Utc};
use serde_json::json;
use uuid::Uuid;
use redis::AsyncCommands;
use std::sync::Arc;
use crate::middleware::auth::Claims;
use crate::db::workout_data::{insert_workout_data, check_duplicate_workout_by_time};
use crate::models::workout_data::WorkoutDataSyncRequest;
use crate::models::common::ApiResponse;
use crate::game::stats_calculator::StatCalculator;
use crate::models::live_game::{LiveGame, LiveGameScoreUpdate};
use crate::services::live_game_service::LiveGameService;
use crate::game::stats_calculator::StatChanges;

#[tracing::instrument(
    name = "Upload workout data with game stats",
    skip(data, pool, redis, live_game_service, claims),
    fields(
        username = %claims.username,
        data_type = %data.device_id
    )
)]
pub async fn upload_workout_data(
    data: web::Json<WorkoutDataSyncRequest>,
    pool: web::Data<sqlx::PgPool>,
    redis: Option<web::Data<Arc<redis::Client>>>,
    live_game_service: Option<web::Data<LiveGameService>>,
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
                tracing::warn!("⚠️ Duplicate workout detected based on time overlap for user: {}. Will store but skip stats.", claims.username);
            }
            is_dup
        }
        Err(e) => {
            tracing::error!("❌ Failed to check for duplicate workouts: {}", e);
            // Continue anyway - assume not duplicate
            false
        }
    };

    // Calculate and apply stats ONLY if not a duplicate
    let stat_changes = if !is_duplicate {
        // 🎲 CALCULATE GAME STATS FROM WORKOUT DATA
        let changes = StatCalculator::calculate_stat_changes(&pool, user_id, &data).await;
        tracing::info!("📊 Calculated stat changes for {}: +{} stamina, +{} strength", 
            claims.username, 
            changes.stamina_change, 
            changes.strength_change, 
        );

        // 💾 APPLY STAT CHANGES TO DATABASE
        let update_result = sqlx::query!(
            r#"
            UPDATE user_avatars 
            SET stamina = stamina + $1, 
                strength = strength + $2
            WHERE user_id = $3
            "#,
            changes.stamina_change,
            changes.strength_change,
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
        
        changes
    } else {
        tracing::info!("⏭️ Skipping stats calculation for duplicate workout");
        // Return zero stats for duplicate
        StatChanges {
            stamina_change: 0,
            strength_change: 0,
            reasoning: vec!["Duplicate workout - stats not applied".to_string()],
            zone_breakdown: None,
        }
    };

    // Insert workout data into database (with duplicate flag)
    tracing::info!("💾 Inserting workout data into database for user: {} with workout_uuid: {:?} (is_duplicate: {})", 
        claims.username, data.workout_uuid, is_duplicate);
    let insert_result = insert_workout_data(&pool, user_id, &data, is_duplicate).await;
    
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

                // 🏆 CHECK FOR ACTIVE LIVE GAMES AND UPDATE SCORES
                if let Some(live_service) = &live_game_service {
                    if let Some(workout_start) = data.workout_start {
                        match check_and_update_live_games(
                            user_id, 
                            &claims.username,
                            sync_id, // Now we have the workout_data_id
                            &stat_changes,
                            &live_service,
                            &workout_start,
                            &pool,
                        ).await {
                            Ok(_) => {
                                tracing::info!("✅ Successfully updated live game scores for user {}", claims.username);
                            }
                            Err(e) => {
                                tracing::error!("❌ Failed to update live game scores for user {}: {}", claims.username, e);
                            }
                        }
                    } else {
                        tracing::warn!("⚠️ No workout start time found for user {}", claims.username);
                    }
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

            // 🎉 ENHANCED RESPONSE WITH GAME STATS
            let (message, sync_data) = if is_duplicate {
                (
                    "Workout data synced (duplicate detected - stats not applied)",
                    json!({
                        "sync_id": sync_id,
                        "timestamp": Utc::now(),
                        "is_duplicate": true,
                        "duplicate_reason": "Similar workout time detected (within 15 seconds)",
                        "game_stats": {
                            "stat_changes": {
                                "stamina_change": 0,
                                "strength_change": 0,
                            },
                            "reasoning": "Duplicate workout - stats not applied to prevent double counting",
                            "summary": "Workout stored but no stats gained (duplicate)"
                        }
                    })
                )
            } else {
                (
                    "Workout data synced and game stats calculated!",
                    json!({
                        "sync_id": sync_id,
                        "timestamp": Utc::now(),
                        "is_duplicate": false,
                        "game_stats": {
                            "stat_changes": {
                                "stamina_change": stat_changes.stamina_change,
                                "strength_change": stat_changes.strength_change,
                            },
                            "reasoning": stat_changes.reasoning,
                            "summary": format!("Gained {} total stat points!", 
                                stat_changes.stamina_change + stat_changes.strength_change
                            )
                        }
                    })
                )
            };

            tracing::info!("✅ Workout data processed successfully for {}: {} (is_duplicate: {})", 
                claims.username, sync_id, is_duplicate);
            HttpResponse::Ok().json(
                ApiResponse::success(message, sync_data)
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

/// Check if user is in any active live games and update scores
async fn check_and_update_live_games(
    user_id: Uuid,
    username: &str,
    workout_data_id: Uuid,
    stat_changes: &StatChanges,
    live_game_service: &LiveGameService,
    workout_start_time: &DateTime<Utc>,
    pool: &sqlx::PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("🎮 Checking for active live games for user {}", username);

    // Get user's active live games
    match live_game_service.get_user_active_games(user_id).await {
        Ok(active_games) => {
            if active_games.is_empty() {
                tracing::debug!("No active live games found for user {}", username);
                return Ok(());
            }

            tracing::info!("🏆 Found {} active live game(s) for user {}", active_games.len(), username);

            for live_game in active_games {
                // Determine which team the user belongs to
                let user_team_id = if let Ok(team_id) = get_user_team_id(user_id, &live_game, pool).await {
                    team_id
                } else {
                    tracing::error!("Could not determine team for user {} in game {}", username, live_game.game_id);
                    continue;
                };
                // Check if the workout start time is within the game start and end times
                if &live_game.game_start_time <= workout_start_time && &live_game.game_end_time >= workout_start_time {
                    tracing::info!("🏆 Workout start time is within the game start and end times for user {}", username);
                    update_live_game_score(user_id, username, user_team_id, stat_changes, live_game_service, &live_game, workout_data_id).await;

                } else {
                    tracing::info!("❌ Workout start time is not within the game start and end times for user {}", username);
                    continue;
                }
            }
            Ok(())
        }
        Err(e) => {
            tracing::error!("❌ Failed to get active games for user {}: {}", username, e);
            return Err(e);
        }
    }
}

async fn update_live_game_score(
    user_id: Uuid,
    username: &str,
    user_team_id: Uuid,
    stat_changes: &StatChanges,
    live_game_service: &LiveGameService,
    live_game: &LiveGame,
    workout_data_id: Uuid,
) {
    tracing::info!("🏆 Updating live game score for user {}", user_id);

    // Calculate score increases based on stat changes
    let score_increase = live_game_service.calculate_score_from_stats(
        stat_changes.stamina_change,
        stat_changes.strength_change,
    );
    let power_increase = live_game_service.calculate_power_from_stats(
        stat_changes.stamina_change,
        stat_changes.strength_change,
    );
    
    tracing::info!("📊 Score calculation for {}: stamina={}, strength={}, score_increase={}, power_increase={}, team_id={}", 
        username, stat_changes.stamina_change, stat_changes.strength_change, 
        score_increase, power_increase, user_team_id);

    // Create the score update
    let score_update = LiveGameScoreUpdate {
        user_id,
        username: username.to_string(),
        team_id: user_team_id,
        score_increase,
        power_increase,
        stamina_gained: stat_changes.stamina_change,
        strength_gained: stat_changes.strength_change,
        description: format!("Workout upload: +{} stamina, +{} strength", 
            stat_changes.stamina_change, stat_changes.strength_change),
        workout_data_id: Some(workout_data_id),
    };

    // Apply the score update
    match live_game_service.handle_score_update(live_game.game_id, score_update).await {
        Ok(updated_game) => {
            tracing::info!("📊 Updated live game {}: {} {} - {} {} (Player: {} +{})", 
                live_game.game_id,
                updated_game.home_team_name,
                updated_game.home_score,
                updated_game.away_score,
                updated_game.away_team_name,
                username,
                score_increase
            );
        }
        Err(e) => {
            tracing::error!("❌ Failed to update live game score for {}: {}", username, e);
        }
    }
 
}

/// Helper function to determine which team a user belongs to in a live game
async fn get_user_team_id(
    user_id: Uuid, 
    live_game: &LiveGame,
    pool: &sqlx::PgPool
) -> Result<Uuid, Box<dyn std::error::Error>> {
    // Simply check team membership directly (no more live_player_contributions complexity!)
    let membership = sqlx::query!(
        r#"
        SELECT team_id 
        FROM team_members 
        WHERE user_id = $1 
        AND status = 'active'
        AND (team_id = $2 OR team_id = $3)
        "#,
        user_id,
        live_game.home_team_id,
        live_game.away_team_id
    )
    .fetch_optional(pool)
    .await?;
    
    match membership {
        Some(m) => Ok(m.team_id),
        None => Err("User does not belong to either team in this game".into())
    }
}

