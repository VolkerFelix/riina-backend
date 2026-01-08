use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::middleware::auth::Claims;
use crate::models::common::ApiResponse;
use crate::models::user::UserStatus;

#[derive(Debug, Deserialize)]
pub struct UpdateUserStatusRequest {
    pub status: UserStatus,
}

#[derive(Debug, Serialize)]
pub struct UserStatusResponse {
    pub user_id: Uuid,
    pub status: UserStatus,
    pub in_player_pool: bool,
}

#[tracing::instrument(
    name = "Update user status",
    skip(pool, claims, request),
    fields(username = %claims.username)
)]
pub async fn update_user_status(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    request: web::Json<UpdateUserStatusRequest>,
) -> HttpResponse {
    let Some(user_id) = claims.user_id() else {
        tracing::error!("Invalid user ID in claims");
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid user ID"));
    };

    let new_status = &request.status;

    tracing::info!(
        "User {} changing status to: {}",
        user_id,
        new_status.to_string()
    );

    // Start a transaction
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("Failed to start transaction: {}", e);
            return HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to update status"));
        }
    };

    // Update user status
    let update_result = sqlx::query!(
        r#"
        UPDATE users
        SET status = $1, updated_at = $2
        WHERE id = $3
        RETURNING status as "status: UserStatus"
        "#,
        new_status.to_string(),
        Utc::now(),
        user_id
    )
    .fetch_one(&mut *tx)
    .await;

    let updated_status = match update_result {
        Ok(record) => record.status,
        Err(e) => {
            tracing::error!("Failed to update user status: {}", e);
            return HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to update status"));
        }
    };

    // Handle player pool logic based on new status
    let in_player_pool = match new_status {
        UserStatus::Active => {
            // Check if user is not in any active team
            let is_in_team = sqlx::query!(
                r#"
                SELECT EXISTS(
                    SELECT 1 FROM team_members
                    WHERE user_id = $1
                    AND status IN ('active', 'pending')
                ) as "exists!"
                "#,
                user_id
            )
            .fetch_one(&mut *tx)
            .await;

            match is_in_team {
                Ok(record) if !record.exists => {
                    // User is not in a team, add to player pool
                    let insert_result = sqlx::query!(
                        r#"
                        INSERT INTO player_pool (user_id, last_active_at)
                        VALUES ($1, $2)
                        ON CONFLICT (user_id)
                        DO UPDATE SET
                            last_active_at = $2,
                            updated_at = $2
                        "#,
                        user_id,
                        Utc::now()
                    )
                    .execute(&mut *tx)
                    .await;

                    match insert_result {
                        Ok(_) => {
                            tracing::info!("User {} added to player pool", user_id);
                            true
                        }
                        Err(e) => {
                            tracing::error!("Failed to add user to player pool: {}", e);
                            // Don't fail the whole operation if player pool update fails
                            false
                        }
                    }
                }
                Ok(_) => {
                    // User is in a team, don't add to player pool
                    tracing::info!("User {} is in a team, not adding to player pool", user_id);
                    false
                }
                Err(e) => {
                    tracing::error!("Failed to check team membership: {}", e);
                    false
                }
            }
        }
        UserStatus::Inactive | UserStatus::Suspended | UserStatus::Banned => {
            // Remove from all team memberships
            let remove_from_teams_result = sqlx::query!(
                r#"
                UPDATE team_members
                SET status = 'inactive', updated_at = $2
                WHERE user_id = $1
                AND status IN ('active', 'pending')
                "#,
                user_id,
                Utc::now()
            )
            .execute(&mut *tx)
            .await;

            match remove_from_teams_result {
                Ok(result) => {
                    let rows_affected = result.rows_affected();
                    if rows_affected > 0 {
                        tracing::info!(
                            "User {} removed from {} team(s)",
                            user_id,
                            rows_affected
                        );
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to remove user from teams: {}", e);
                    // Rollback will happen if we return error
                    return HttpResponse::InternalServerError()
                        .json(ApiResponse::<()>::error("Failed to remove user from teams"));
                }
            }

            // Remove from player pool
            let delete_result = sqlx::query!(
                r#"
                DELETE FROM player_pool
                WHERE user_id = $1
                "#,
                user_id
            )
            .execute(&mut *tx)
            .await;

            match delete_result {
                Ok(_) => {
                    tracing::info!("User {} removed from player pool", user_id);
                }
                Err(e) => {
                    tracing::error!("Failed to remove user from player pool: {}", e);
                    // Don't fail the whole operation
                }
            }
            false
        }
    };

    // Commit the transaction
    if let Err(e) = tx.commit().await {
        tracing::error!("Failed to commit transaction: {}", e);
        return HttpResponse::InternalServerError()
            .json(ApiResponse::<()>::error("Failed to update status"));
    }

    let response = UserStatusResponse {
        user_id,
        status: updated_status,
        in_player_pool,
    };

    HttpResponse::Ok().json(ApiResponse::success("Status updated successfully", response))
}

#[tracing::instrument(
    name = "Get user status",
    skip(pool, claims),
    fields(username = %claims.username)
)]
pub async fn get_user_status(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> HttpResponse {
    let Some(user_id) = claims.user_id() else {
        tracing::error!("Invalid user ID in claims");
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid user ID"));
    };

    // Get user status and check if in player pool
    let result = sqlx::query!(
        r#"
        SELECT
            u.status as "status: UserStatus",
            EXISTS(SELECT 1 FROM player_pool WHERE user_id = u.id) as "in_player_pool!"
        FROM users u
        WHERE u.id = $1
        "#,
        user_id
    )
    .fetch_optional(&**pool)
    .await;

    match result {
        Ok(Some(record)) => {
            let response = UserStatusResponse {
                user_id,
                status: record.status,
                in_player_pool: record.in_player_pool,
            };
            HttpResponse::Ok().json(ApiResponse::success("User status retrieved successfully", response))
        }
        Ok(None) => {
            HttpResponse::NotFound().json(ApiResponse::<()>::error("User not found"))
        }
        Err(e) => {
            tracing::error!("Failed to fetch user status: {}", e);
            HttpResponse::InternalServerError()
                .json(ApiResponse::<()>::error("Failed to fetch user status"))
        }
    }
}
