use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;

use crate::middleware::auth::Claims;
use crate::models::common::ApiResponse;
use crate::models::team_invitation::{
    InvitationStatus, SendInvitationRequest, RespondToInvitationRequest,
    TeamInvitationWithDetails,
};
use crate::models::team::TeamRole;
use crate::league::constants::MAX_TEAM_SIZE;
use crate::services::social_events::send_notification_to_user;

/// Send a team invitation to a free agent
#[tracing::instrument(
    name = "Send team invitation",
    skip(pool, claims, team_id, request, redis_client),
    fields(username = %claims.username, team_id = %team_id)
)]
pub async fn send_invitation(
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<redis::Client>>,
    claims: web::ReqData<Claims>,
    team_id: web::Path<Uuid>,
    request: web::Json<SendInvitationRequest>,
) -> HttpResponse {
    let team_id = team_id.into_inner();
    let inviter_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid user ID"));
        }
    };

    // Check if inviter is team owner or admin
    let member_check = sqlx::query!(
        r#"
        SELECT role as "role: TeamRole"
        FROM team_members
        WHERE team_id = $1 AND user_id = $2 AND status = 'active'
        "#,
        team_id,
        inviter_id
    )
    .fetch_optional(pool.get_ref())
    .await;

    match member_check {
        Ok(Some(member)) => {
            if member.role != TeamRole::Owner && member.role != TeamRole::Admin {
                return HttpResponse::Forbidden().json(ApiResponse::<()>::error(
                    "Only team owners and admins can send invitations"
                ));
            }
        }
        Ok(None) => {
            return HttpResponse::Forbidden().json(ApiResponse::<()>::error(
                "You are not a member of this team"
            ));
        }
        Err(e) => {
            tracing::error!("Database error checking team membership: {}", e);
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to verify team membership"
            ));
        }
    }

    // Check if invitee exists and is a free agent (in player pool)
    let invitee_check = sqlx::query!(
        r#"
        SELECT pp.user_id
        FROM player_pool pp
        INNER JOIN users u ON pp.user_id = u.id
        WHERE pp.user_id = $1 AND u.status = 'active'
        "#,
        request.invitee_id
    )
    .fetch_optional(pool.get_ref())
    .await;

    match invitee_check {
        Ok(None) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                "User is not a free agent or does not exist"
            ));
        }
        Err(e) => {
            tracing::error!("Database error checking invitee status: {}", e);
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to verify invitee status"
            ));
        }
        _ => {}
    }

    // Check if team already has maximum members
    let member_count_result = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM team_members
        WHERE team_id = $1 AND status = 'active'
        "#,
        team_id
    )
    .fetch_one(pool.get_ref())
    .await;

    match member_count_result {
        Ok(row) => {
            if let Some(count) = row.count {
                if count >= MAX_TEAM_SIZE {
                    return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
                        &format!("Your team is already full (maximum {} members)", MAX_TEAM_SIZE)
                    ));
                }
            }
        }
        Err(e) => {
            tracing::error!("Database error checking team size: {}", e);
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to verify team size"
            ));
        }
    }

    // Check if there's already a pending invitation
    let existing_invitation = sqlx::query!(
        r#"
        SELECT id
        FROM team_invitations
        WHERE team_id = $1 AND invitee_id = $2 AND status = 'pending'
        "#,
        team_id,
        request.invitee_id
    )
    .fetch_optional(pool.get_ref())
    .await;

    match existing_invitation {
        Ok(Some(inv)) => {
            // Return success with the existing invitation ID instead of an error
            tracing::info!(
                "Pending invitation already exists: team_id={}, invitee_id={}, invitation_id={}",
                team_id,
                request.invitee_id,
                inv.id
            );
            return HttpResponse::Ok().json(ApiResponse::success(
                "An invitation to this user is already pending",
                serde_json::json!({ "invitation_id": inv.id })
            ));
        }
        Err(e) => {
            tracing::error!("Database error checking existing invitation: {}", e);
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to check existing invitations"
            ));
        }
        _ => {}
    }

    // Create the invitation
    let result = sqlx::query!(
        r#"
        INSERT INTO team_invitations (team_id, inviter_id, invitee_id, status, message)
        VALUES ($1, $2, $3, 'pending', $4)
        RETURNING id
        "#,
        team_id,
        inviter_id,
        request.invitee_id,
        request.message
    )
    .fetch_one(pool.get_ref())
    .await;

    match result {
        Ok(row) => {
            let invitation_id = row.id;

            tracing::info!(
                "Team invitation sent: team_id={}, inviter_id={}, invitee_id={}, invitation_id={}",
                team_id,
                inviter_id,
                request.invitee_id,
                invitation_id
            );

            // Create notification for the invitee
            // Fetch team name and inviter username
            let notification_data = sqlx::query!(
                r#"
                SELECT t.team_name, u.username as inviter_username
                FROM teams t
                CROSS JOIN users u
                WHERE t.id = $1 AND u.id = $2
                "#,
                team_id,
                inviter_id
            )
            .fetch_optional(pool.get_ref())
            .await;

            if let Ok(Some(data)) = notification_data {
                let message = format!(
                    "{} invited you to join {}",
                    data.inviter_username,
                    data.team_name
                );

                // Save notification to database
                match sqlx::query!(
                    r#"
                    INSERT INTO notifications (recipient_id, actor_id, notification_type, entity_type, entity_id, message)
                    VALUES ($1, $2, 'team_invitation', 'invitation', $3, $4)
                    RETURNING id
                    "#,
                    request.invitee_id,
                    inviter_id,
                    invitation_id,
                    message
                )
                .fetch_one(pool.get_ref())
                .await {
                    Ok(notification_row) => {
                        let notification_id = notification_row.id;
                        tracing::info!("Created notification {} for team invitation", notification_id);

                        // Send notification to user via WebSocket
                        if let Err(e) = send_notification_to_user(
                            &redis_client,
                            request.invitee_id,
                            notification_id,
                            data.inviter_username.clone(),
                            "team_invitation".to_string(),
                            message.clone(),
                        ).await {
                            tracing::error!("Failed to send team invitation notification to user: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to create team invitation notification: {}", e);
                    }
                }
            } else {
                tracing::error!("Failed to fetch team/inviter data for notification");
            }

            HttpResponse::Created().json(ApiResponse::success(
                "Invitation sent successfully",
                serde_json::json!({ "invitation_id": invitation_id })
            ))
        }
        Err(e) => {
            tracing::error!("Failed to create team invitation: {}", e);
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to send invitation"
            ))
        }
    }
}

/// Get all invitations for the current user
#[tracing::instrument(
    name = "Get user invitations",
    skip(pool, claims),
    fields(username = %claims.username)
)]
pub async fn get_user_invitations(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> HttpResponse {
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid user ID"));
        }
    };

    let result = sqlx::query_as!(
        TeamInvitationWithDetails,
        r#"
        SELECT
            ti.id,
            ti.team_id,
            t.team_name,
            t.team_color,
            ti.inviter_id,
            u_inviter.username as inviter_username,
            ti.invitee_id,
            u_invitee.username as invitee_username,
            ti.status as "status: InvitationStatus",
            ti.message,
            ti.created_at,
            ti.responded_at
        FROM team_invitations ti
        INNER JOIN teams t ON ti.team_id = t.id
        INNER JOIN users u_inviter ON ti.inviter_id = u_inviter.id
        INNER JOIN users u_invitee ON ti.invitee_id = u_invitee.id
        WHERE ti.invitee_id = $1
        ORDER BY
            CASE WHEN ti.status = 'pending' THEN 0 ELSE 1 END,
            ti.created_at DESC
        "#,
        user_id
    )
    .fetch_all(pool.get_ref())
    .await;

    match result {
        Ok(invitations) => {
            HttpResponse::Ok().json(ApiResponse::success(
                "Invitations retrieved successfully",
                serde_json::json!({
                    "invitations": invitations,
                    "total_count": invitations.len()
                })
            ))
        }
        Err(e) => {
            tracing::error!("Failed to fetch invitations: {}", e);
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to fetch invitations"
            ))
        }
    }
}

/// Respond to a team invitation (accept or decline)
#[tracing::instrument(
    name = "Respond to team invitation",
    skip(pool, claims, invitation_id, request),
    fields(username = %claims.username, invitation_id = %invitation_id)
)]
pub async fn respond_to_invitation(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    invitation_id: web::Path<Uuid>,
    request: web::Json<RespondToInvitationRequest>,
) -> HttpResponse {
    let invitation_id = invitation_id.into_inner();
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid user ID"));
        }
    };

    // Get invitation details
    let invitation = sqlx::query!(
        r#"
        SELECT team_id, invitee_id, status as "status: InvitationStatus"
        FROM team_invitations
        WHERE id = $1
        "#,
        invitation_id
    )
    .fetch_optional(pool.get_ref())
    .await;

    let invitation = match invitation {
        Ok(Some(inv)) => inv,
        Ok(None) => {
            return HttpResponse::NotFound().json(ApiResponse::<()>::error(
                "Invitation not found"
            ));
        }
        Err(e) => {
            tracing::error!("Database error fetching invitation: {}", e);
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to fetch invitation"
            ));
        }
    };

    // Verify the user is the invitee
    if invitation.invitee_id != user_id {
        return HttpResponse::Forbidden().json(ApiResponse::<()>::error(
            "You can only respond to your own invitations"
        ));
    }

    // Check if invitation is still pending
    if invitation.status != InvitationStatus::Pending {
        return HttpResponse::BadRequest().json(ApiResponse::<()>::error(
            "This invitation has already been responded to"
        ));
    }

    let new_status = if request.accept { "accepted" } else { "declined" };

    // Start transaction
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("Failed to start transaction: {}", e);
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to process invitation response"
            ));
        }
    };

    // Update invitation status
    let update_result = sqlx::query!(
        r#"
        UPDATE team_invitations
        SET status = $1::varchar, responded_at = NOW()
        WHERE id = $2
        "#,
        new_status,
        invitation_id
    )
    .execute(&mut *tx)
    .await;

    if let Err(e) = update_result {
        tracing::error!("Failed to update invitation status: {}", e);
        let _ = tx.rollback().await;
        return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
            "Failed to update invitation"
        ));
    }

    // If accepted, add user to team and remove from player pool
    if request.accept {
        // Add to team
        let add_result = sqlx::query!(
            r#"
            INSERT INTO team_members (team_id, user_id, role, status)
            VALUES ($1, $2, 'member', 'active')
            "#,
            invitation.team_id,
            user_id
        )
        .execute(&mut *tx)
        .await;

        if let Err(e) = add_result {
            tracing::error!("Failed to add user to team: {}", e);
            let _ = tx.rollback().await;
            return HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to join team"
            ));
        }

        // Remove from player pool
        let remove_result = sqlx::query!(
            r#"
            DELETE FROM player_pool WHERE user_id = $1
            "#,
            user_id
        )
        .execute(&mut *tx)
        .await;

        if let Err(e) = remove_result {
            tracing::warn!("Failed to remove user from player pool: {}", e);
            // Don't fail the transaction, just log
        }
    }

    // Commit transaction
    match tx.commit().await {
        Ok(_) => {
            let action = if request.accept { "accepted" } else { "declined" };
            tracing::info!("Invitation {} by user {}", action, user_id);

            HttpResponse::Ok().json(ApiResponse::success(
                format!("Invitation {} successfully", action),
                serde_json::json!({})
            ))
        }
        Err(e) => {
            tracing::error!("Failed to commit transaction: {}", e);
            HttpResponse::InternalServerError().json(ApiResponse::<()>::error(
                "Failed to process invitation response"
            ))
        }
    }
}
