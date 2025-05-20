use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;
use crate::middleware::auth::Claims;
use crate::db::health_data::insert_health_data;
use crate::models::health_data::{HealthDataSyncRequest, HealthDataSyncResponse};
use redis::AsyncCommands;

#[tracing::instrument(
    name = "Upload health data",
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
    tracing::info!("Sync health data handler called from device: {}", data.device_id);
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
    // Insert health data into database
    let insert_result = insert_health_data(&pool, user_id, &data).await;
    
    match insert_result {
        Ok(sync_id) => {
            // Identify the type of data for event classification
            let data_types = determine_data_types(&data);
            tracing::info!("Data types detected: {:?}", data_types);

            // Publish separate events based on the data types present
            for data_type in &data_types {
                let event_type = match data_type.as_str() {
                    "sleep" => "sleep_data_uploaded",
                    "steps" => "activity_data_uploaded",
                    "heart_rate" => "vitals_data_uploaded",
                    _ => "health_data_uploaded",
                };
                
                let event = serde_json::json!({
                    "event_type": event_type,
                    "user_id": user_id.to_string(),
                    "sync_id": sync_id.to_string(),
                    "timestamp": Utc::now().to_rfc3339()
                });

                let message_str = serde_json::to_string(&event)
                    .unwrap_or_else(|e| {
                        tracing::error!("Failed to serialize Redis message: {}", e);
                        "{}".to_string()
                    });

                if let Some(redis_client) = &redis {
                    match redis_client.get_async_connection().await {
                        Ok(mut conn) => {
                            let pub_result: Result<i32, redis::RedisError> = conn.publish("evolveme:events:health_data", message_str).await;
                            match pub_result {
                                Ok(receivers) => {
                                    tracing::info!("Successfully published {} event for sync_id: {} to {} receivers", event_type, sync_id, receivers);
                                }
                                Err(e) => {
                                    tracing::error!("Failed to publish {} event: {}", event_type, e);
                                }
                            }
                        },
                        Err(e) => {
                            tracing::error!("Failed to get Redis connection: {}", e);
                        }
                    }
                }
            }

            // Publish event to Redis for global health data events
            let global_event = serde_json::json!({
                "event_type": "health_data_uploaded",
                "user_id": user_id.to_string(),
                "sync_id": sync_id.to_string(),
                "timestamp": Utc::now().to_rfc3339()
            });

            let global_message_str = serde_json::to_string(&global_event)
                .unwrap_or_else(|e| {
                    tracing::error!("Failed to serialize Redis message: {}", e);
                    "{}".to_string()
                });

            if let Some(redis_client) = &redis {
                match redis_client.get_async_connection().await {
                    Ok(mut conn) => {
                        let pub_result: Result<i32, redis::RedisError> = conn.publish("evolveme:events:health_data", global_message_str).await;
                        match pub_result {
                            Ok(receivers) => {
                                tracing::info!("Successfully published health data event for sync_id: {} to {} receivers", sync_id, receivers);
                            }
                            Err(e) => {
                                tracing::error!("Failed to publish health data event: {}", e);
                            }
                        }
                    },
                    Err(e) => {
                        tracing::error!("Failed to get Redis connection: {}", e);
                    }
                }
            }
            
            // Publish event to user-specific Redis channel for real-time notification
            match publish_user_notification(redis, user_id, sync_id, &claims.username, &data_types).await {
                Ok(_) => {
                    tracing::info!("Successfully published user notification for sync_id: {}", sync_id);
                },
                Err(e) => {
                    tracing::error!("Failed to publish user notification: {}", e);
                }
            }

            // Prepare successful response
            let response = HealthDataSyncResponse {
                success: true,
                message: "Health data synced successfully".to_string(),
                sync_id,
                timestamp: Utc::now(),
            };
            tracing::info!("Health data synced successfully: {}", sync_id); 
            HttpResponse::Ok().json(response)
        }
        Err(e) => {
            // Prepare error response
            let response = HealthDataSyncResponse {
                success: false,
                message: format!("Failed to sync health data: {}", e),
                sync_id: uuid::Uuid::nil(), // Use nil UUID for error case
                timestamp: Utc::now(),
            };
            tracing::error!("Failed to sync health data: {}", e);
            HttpResponse::InternalServerError().json(response)
        }
    }
}

async fn publish_user_notification(
    redis: Option<web::Data<redis::Client>>,
    user_id: Uuid,
    sync_id: Uuid,
    username: &str,
    data_types: &[String]
) -> Result<(), redis::RedisError> {
    let redis_client = match redis {
        Some(client) => client,
        None => {
            tracing::info!("Redis not available - skipping user notification");
            return Ok(());
        }
    };

    let mut conn = redis_client.get_async_connection().await?;
    
    // Create notification event with details needed by the frontend
    let notification = serde_json::json!({
        "event_type": "new_health_data",
        "user_id": user_id.to_string(),
        "username": username,
        "sync_id": sync_id.to_string(),
        "message": "New health data available.",
        "data_types": data_types,
        "timestamp": Utc::now().to_rfc3339()
    });

    let message_str = serde_json::to_string(&notification)
    .unwrap_or_else(|e| {
        tracing::error!("Failed to serialize Redis message: {}", e);
        "{}".to_string()
    });

    // Publish to the user-specific channel
    let channel = format!("evolveme:events:user:{}", user_id.to_string());
    let pub_result: Result<i32, redis::RedisError> = conn.publish(&channel, message_str).await;
    match pub_result {
        Ok(receivers) => {
            tracing::info!("Successfully published user notification for sync_id: {} to {} receivers", sync_id, receivers);
        }
        Err(e) => {
            tracing::error!("Failed to publish user notification: {}", e);
        }
    }
    Ok(())
}

fn determine_data_types(data: &HealthDataSyncRequest) -> Vec<String> {
    let mut data_types = Vec::new();
    
    if data.sleep.is_some() {
        data_types.push("sleep".to_string());
    }
    
    if data.steps.is_some() && data.steps.unwrap() > 0 {
        data_types.push("steps".to_string());
    }
    
    if data.heart_rate.is_some() {
        data_types.push("heart_rate".to_string());
    }
    
    if data.active_energy_burned.is_some() {
        data_types.push("energy".to_string());
    }
    
    if data.additional_metrics.is_some() {
        // Extract metrics from additional_metrics
        if let Some(ref metrics) = data.additional_metrics {
            let json_value = serde_json::to_value(metrics).unwrap_or(serde_json::Value::Null);
            
            if let serde_json::Value::Object(obj) = json_value {
                // Check for specific additional metrics we care about
                if obj.contains_key("blood_oxygen") {
                    data_types.push("blood_oxygen".to_string());
                }
                
                if obj.contains_key("respiratory_rate") {
                    data_types.push("respiratory".to_string());
                }
                
                if obj.contains_key("hrv") {
                    data_types.push("hrv".to_string());
                }
                
                if obj.contains_key("stress_level") {
                    data_types.push("stress".to_string());
                }
            }
        }
    }
    
    // If no specific types are detected, add a generic "health" type
    if data_types.is_empty() {
        data_types.push("health".to_string());
    }
    
    data_types
}