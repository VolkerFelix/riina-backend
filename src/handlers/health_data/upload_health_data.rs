// Enhanced src/handlers/health_data/upload_health_data.rs - Now with game stats!

use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;
use crate::middleware::auth::Claims;
use crate::db::health_data::insert_health_data;
use crate::models::health_data::HealthDataSyncRequest;
use crate::game::stats_calculator::StatCalculator;
use redis::AsyncCommands;

#[tracing::instrument(
    name = "Upload health data with game stats",
    skip(data, pool, redis, claims),
    fields(
        username = %claims.username,
        data_type = %data.device_id
    )
)]
pub async fn upload_health_data(
    data: web::Json<HealthDataSyncRequest>,
    pool: web::Data<sqlx::PgPool>,
    redis: Option<web::Data<redis::Client>>,
    claims: web::ReqData<Claims>
) -> HttpResponse {
    tracing::info!("🎮 Processing health data with game mechanics for user: {}", claims.username);
    
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => {
            tracing::info!("User ID parsed successfully: {}", id);
            id
        },
        Err(e) => {
            tracing::error!("Failed to parse user ID: {}", e);
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": "Invalid user ID"
            }));
        }
    };

    // 🎲 CALCULATE GAME STATS FROM HEALTH DATA
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
            strength = strength + $2,
            experience_points = experience_points + $3
        WHERE user_id = $4
        "#,
        stat_changes.stamina_change,
        stat_changes.strength_change,
        (stat_changes.stamina_change + stat_changes.strength_change) as i64, // Experience based on total stat gain
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
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": "Failed to update avatar stats"
            }));
        }
    }

    // Insert health data into database
    let insert_result = insert_health_data(&pool, user_id, &data).await;
    
    match insert_result {
        Ok(sync_id) => {
            // 🎯 PREPARE GAME EVENT FOR REAL-TIME NOTIFICATION
            let game_event = json!({
                "event_type": "health_data_processed",
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
            let response = json!({
                "success": true,
                "message": "Health data synced and game stats calculated!",
                "sync_id": sync_id,
                "timestamp": Utc::now(),
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
            });

            tracing::info!("✅ Health data processed successfully with game mechanics for {}: {}", 
                claims.username, sync_id);
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            let response = json!({
                "success": false,
                "message": format!("Failed to sync health data: {}", e),
                "sync_id": null,
                "timestamp": Utc::now()
            });
            tracing::error!("❌ Failed to sync health data for {}: {}", claims.username, e);
            HttpResponse::InternalServerError().json(response)
        }
    }
}