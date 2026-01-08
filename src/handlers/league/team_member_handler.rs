use actix_web::{web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;
use serde_json::json;
use std::sync::Arc;

use crate::middleware::auth::Claims;
use crate::models::team::*;
use crate::models::common::ApiResponse;
use crate::handlers::league::team_member_helper::*;
use crate::models::user::UserRole;
use crate::services::player_pool_events;

/// Add a user to a team
#[tracing::instrument(
    name = "Add team member",
    skip(request, pool, claims),
    fields(
        user = %claims.username,
        team_id = %team_id
    )
)]
pub async fn add_team_member(
    team_id: web::Path<Uuid>,
    request: web::Json<AddTeamMemberRequest>,
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<redis::Client>>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let team_id = team_id.into_inner();

    tracing::info!("Adding member(s) to team {}", team_id);

    // Validate the request
    if let Err(validation_error) = request.validate() {
        tracing::warn!("Add team member validation failed: {}", validation_error);
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(validation_error)));
    }

    let Some(requester_id) = claims.user_id() else {
        tracing::error!("Invalid user ID in claims");
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid user ID")));
    };

    // Check if requester has permission to add members (must be owner or admin)
    let requester_role = match check_team_member_role(&team_id, &requester_id, &pool).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            return Ok(HttpResponse::Forbidden().json(ApiResponse::<()>::error("You are not a member of this team")));
        }
        Err(e) => {
            tracing::error!("Failed to check requester role: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to verify permissions")));
        }
    };

    if requester_role != TeamRole::Owner && requester_role != TeamRole::Admin {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::<()>::error("Only team owners and admins can add members")));
    }

    // Get team info for notifications
    let team_info = match get_team_info(&team_id, &pool).await {
        Ok(Some(team)) => team,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::<()>::error("Team not found")));
        }
        Err(e) => {
            tracing::error!("Failed to get team info: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to get team information")));
        }
    };

    let mut added_members = Vec::new();
    let mut errors = Vec::new();

    for member in &request.member_request {
        match add_member(team_id, member, &pool, &requester_role).await {
            Ok(member_info) => {
                // Remove member from player pool using common helper
                let _ = remove_from_player_pool(&member_info.user_id, pool.get_ref()).await;

                // Publish player_left event (left the pool)
                if let Err(e) = player_pool_events::publish_player_left(
                    &redis_client,
                    &pool,
                    member_info.user_id,
                    member_info.username.clone(),
                    None, // league_id
                ).await {
                    tracing::warn!("Failed to publish player_left event: {}", e);
                }

                // Publish player_assigned event
                if let Err(e) = player_pool_events::publish_player_assigned(
                    &redis_client,
                    &pool,
                    member_info.user_id,
                    member_info.username.clone(),
                    None, // league_id - could be added if needed
                    team_id,
                    team_info.team_name.clone(),
                ).await {
                    tracing::warn!("Failed to publish player_assigned event: {}", e);
                    // Don't fail the operation if notification fails
                }

                added_members.push(member_info);
            }
            Err(e) => {
                tracing::error!("Failed to add member: {}", e);
                errors.push(e.to_string());
            }
        }
    }

    // Return appropriate response based on results
    if added_members.is_empty() && !errors.is_empty() {
        // If we have errors and no successful additions
        Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(errors.join(", "))))
    } else if !added_members.is_empty() {
        // If we have at least one successful addition
        Ok(HttpResponse::Created().json(ApiResponse::success(
            if errors.is_empty() {
                "All users added to team successfully".to_string()
            } else {
                format!("Some users added successfully. Errors: {}", errors.join(", "))
            },
            json!({"members": added_members})
        )))
    } else {
        // If we somehow got here with no additions and no errors
        Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error("No members were added")))
    }
}

/// Get all members of a team
pub async fn get_team_members(
    team_id: web::Path<Uuid>,
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let team_id = team_id.into_inner();

    let Some(requester_id) = claims.user_id() else {
        tracing::error!("Invalid user ID in claims");
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid user ID")));
    };

    // Check if requester is a member of the team or an admin (regardless of status)
    let is_admin = matches!(claims.role, UserRole::Admin);

    if !is_admin {
        // Check membership without status filter so inactive members can still view the team
        let is_member = match sqlx::query!(
            r#"
            SELECT id FROM team_members
            WHERE team_id = $1 AND user_id = $2
            "#,
            team_id,
            requester_id
        )
        .fetch_optional(pool.get_ref())
        .await
        {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(e) => {
                tracing::error!("Failed to check requester membership: {}", e);
                return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to verify membership")));
            }
        };

        if !is_member {
            return Ok(HttpResponse::Forbidden().json(ApiResponse::<()>::error("You must be a team member to view the member list")));
        }
    }

    // Get team info and members
    let team_info = match get_team_info(&team_id, &pool).await {
        Ok(Some(team)) => team,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::<()>::error("Team not found")));
        }
        Err(e) => {
            tracing::error!("Failed to get team info: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to get team information")));
        }
    };

    match sqlx::query_as!(
        TeamMemberInfo,
        r#"
        SELECT 
            tm.id,
            tm.team_id,
            tm.user_id,
            u.username,
            u.email,
            tm.role as "role: TeamRole",
            tm.status as "status: MemberStatus",
            tm.joined_at,
            tm.updated_at
        FROM team_members tm
        JOIN users u ON tm.user_id = u.id
        WHERE tm.team_id = $1
        ORDER BY 
            CASE tm.role 
                WHEN 'owner' THEN 1
                WHEN 'admin' THEN 2
                WHEN 'member' THEN 3
            END,
            tm.joined_at ASC
        "#,
        team_id
    )
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(members) => {
            Ok(HttpResponse::Ok().json(ApiResponse::success(
                "Team members retrieved successfully",
                TeamWithMembers {
                    team: team_info,
                    members: members.clone(),
                    member_count: members.len(),
                }
            )))
        }
        Err(e) => {
            tracing::error!("Failed to get team members for team {}: {}", team_id, e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to get team members")))
        }
    }
}

/// Remove a user from a team
pub async fn remove_team_member(
    path: web::Path<(Uuid, Uuid)>, // (team_id, user_id)
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<redis::Client>>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let (team_id, target_user_id) = path.into_inner();

    let Some(requester_id) = claims.user_id() else {
        tracing::error!("Invalid user ID in claims");
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid user ID")));
    };

    // Get requester's role
    let requester_role = match check_team_member_role(&team_id, &requester_id, &pool).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            return Ok(HttpResponse::Forbidden().json(ApiResponse::<()>::error("You are not a member of this team")));
        }
        Err(e) => {
            tracing::error!("Failed to check requester role: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to verify permissions")));
        }
    };

    // Get target user's role
    let target_role = match check_team_member_role(&team_id, &target_user_id, &pool).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::<()>::error("User is not a member of this team")));
        }
        Err(e) => {
            tracing::error!("Failed to check target user role: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to verify target user")));
        }
    };

    // Check permissions
    if requester_id == target_user_id {
        // Users can always remove themselves (leave team)
        // But owners need special handling (can't leave if they're the last owner)
    } else {
        // Removing someone else requires admin/owner privileges
        if requester_role != TeamRole::Owner && requester_role != TeamRole::Admin {
            return Ok(HttpResponse::Forbidden().json(ApiResponse::<()>::error("Only team owners and admins can remove members")));
        }

        // Admins can't remove owners
        if requester_role == TeamRole::Admin && target_role == TeamRole::Owner {
            return Ok(HttpResponse::Forbidden().json(ApiResponse::<()>::error("Admins cannot remove team owners")));
        }
    }

    // Check if we're trying to remove the last owner
    if target_role == TeamRole::Owner {
        match count_team_owners(&team_id, &pool).await {
            Ok(count) if count <= 1 => {
                return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error("Cannot remove the last owner from a team")));
            }
            Ok(_) => {
                // There are other owners, allow the removal
            }
            Err(e) => {
                tracing::error!("Failed to count team owners: {}", e);
                return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to verify team ownership")));
            }
        }
    }

    // Get user info before removal for notifications
    let user_info = match sqlx::query!(
        r#"
        SELECT username
        FROM users
        WHERE id = $1
        "#,
        target_user_id
    )
    .fetch_one(pool.get_ref())
    .await
    {
        Ok(user) => user,
        Err(e) => {
            tracing::error!("Failed to get user info: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to get user information")));
        }
    };

    // Get team info before removal for notifications
    let team_info = match sqlx::query!(
        r#"
        SELECT team_name
        FROM teams
        WHERE id = $1
        "#,
        team_id
    )
    .fetch_one(pool.get_ref())
    .await
    {
        Ok(team) => team,
        Err(e) => {
            tracing::error!("Failed to get team info: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to get team information")));
        }
    };

    // Use common function to remove member and return to pool
    match remove_member_and_return_to_pool(&team_id, &target_user_id, pool.get_ref()).await {
        Ok(_) => {
            // Publish player_left_team event (left the team)
            if let Err(e) = player_pool_events::publish_player_left_team(
                &redis_client,
                &pool,
                target_user_id,
                user_info.username.clone(),
                None, // league_id
                team_id,
                team_info.team_name.clone(),
            ).await {
                tracing::warn!("Failed to publish player_left_team event: {}", e);
            }

            // Publish player_joined event (joined the pool)
            if let Err(e) = player_pool_events::publish_player_joined(
                &redis_client,
                &pool,
                target_user_id,
                user_info.username.clone(),
                None, // league_id
            ).await {
                tracing::warn!("Failed to publish player_joined event: {}", e);
            }

            Ok(HttpResponse::Ok().json(ApiResponse::<()>::success_message("User removed from team successfully")))
        }
        Err(e) => {
            tracing::error!("Failed to remove user {} from team {}: {}", target_user_id, team_id, e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error(&format!("Failed to remove user from team: {}", e))))
        }
    }
}

/// Update current user's status in their team (active/inactive)
pub async fn update_my_team_status(
    team_id: web::Path<Uuid>,
    request: web::Json<serde_json::Value>,
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let team_id = team_id.into_inner();

    // Parse the status from request
    let status_str = match request.get("status").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error("Missing 'status' field")));
        }
    };

    // Validate status (only allow active/inactive)
    let new_status = match status_str {
        "active" => MemberStatus::Active,
        "inactive" => MemberStatus::Inactive,
        _ => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error("Status must be 'active' or 'inactive'")));
        }
    };

    let Some(user_id) = claims.user_id() else {
        tracing::error!("Invalid user ID in claims");
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid user ID")));
    };

    // Check if user is a member of this team (regardless of status, since they need to be able to reactivate)
    let is_member = match sqlx::query!(
        r#"
        SELECT id FROM team_members
        WHERE team_id = $1 AND user_id = $2
        "#,
        team_id,
        user_id
    )
    .fetch_optional(pool.get_ref())
    .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => {
            tracing::error!("Failed to check membership: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to verify membership")));
        }
    };

    if !is_member {
        return Ok(HttpResponse::Forbidden().json(ApiResponse::<()>::error("You are not a member of this team")));
    }

    // Update status
    match sqlx::query!(
        r#"
        UPDATE team_members
        SET status = $1, updated_at = NOW()
        WHERE team_id = $2 AND user_id = $3
        "#,
        new_status.to_string(),
        team_id,
        user_id
    )
    .execute(pool.get_ref())
    .await
    {
        Ok(_) => {
            tracing::info!("User {} updated status to {} in team {}", user_id, new_status, team_id);
            Ok(HttpResponse::Ok().json(ApiResponse::success(
                format!("Status updated to {}", new_status),
                json!({"status": new_status})
            )))
        }
        Err(e) => {
            tracing::error!("Failed to update status: {}", e);
            Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to update status")))
        }
    }
}

/// Update a team member's role or status
pub async fn update_team_member(
    path: web::Path<(Uuid, Uuid)>, // (team_id, user_id)
    request: web::Json<UpdateTeamMemberRequest>,
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let (team_id, target_user_id) = path.into_inner();
    
    // Validate the request
    if let Err(validation_error) = request.validate() {
        tracing::warn!("Update team member validation failed: {}", validation_error);
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error(validation_error)));
    }

    let Some(requester_id) = claims.user_id() else {
        tracing::error!("Invalid user ID in claims");
        return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error("Invalid user ID")));
    };

    // Get requester's role
    let requester_role = match check_team_member_role(&team_id, &requester_id, &pool).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            return Ok(HttpResponse::Forbidden().json(ApiResponse::<()>::error("You are not a member of this team")));
        }
        Err(e) => {
            tracing::error!("Failed to check requester role: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to verify permissions")));
        }
    };

    // Check if target user exists in team
    let target_role = match check_team_member_role(&team_id, &target_user_id, &pool).await {
        Ok(Some(role)) => role,
        Ok(None) => {
            return Ok(HttpResponse::NotFound().json(ApiResponse::<()>::error("User is not a member of this team")));
        }
        Err(e) => {
            tracing::error!("Failed to check target user role: {}", e);
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to verify target user")));
        }
    };

    // Check permissions for role changes
    if let Some(new_role) = &request.role {
        // Only owners can change roles
        if requester_role != TeamRole::Owner {
            return Ok(HttpResponse::Forbidden().json(ApiResponse::<()>::error("Only team owners can change member roles")));
        }

        // Can't change your own role if you're the last owner
        if requester_id == target_user_id && target_role == TeamRole::Owner && *new_role != TeamRole::Owner {
            // Check if there are other owners
            match count_team_owners(&team_id, &pool).await {
                Ok(count) if count <= 1 => {
                    return Ok(HttpResponse::BadRequest().json(ApiResponse::<()>::error("Cannot change the role of the last owner")));
                }
                Ok(_) => {
                    // There are other owners, allow the change
                }
                Err(e) => {
                    tracing::error!("Failed to count team owners: {}", e);
                    return Ok(HttpResponse::InternalServerError().json(ApiResponse::<()>::error("Failed to verify team ownership")));
                }
            }
        }
    }

    // Update the member
    match sqlx::query!(
        r#"
        UPDATE team_members 
        SET 
            role = COALESCE($1, role),
            status = COALESCE($2, status),
            updated_at = NOW()
        WHERE team_id = $3 AND user_id = $4
        "#,
        request.role.as_ref().map(|r| r.to_string()),
        request.status.as_ref().map(|s| s.to_string()),
        team_id,
        target_user_id
    )
    .execute(pool.get_ref())
    .await
    {
        Ok(result) => {
            if result.rows_affected() > 0 {
                tracing::info!("Successfully updated member {} in team {}", target_user_id, team_id);
                
                // Get updated member info
                match get_team_member_info(&team_id, &target_user_id, &pool).await {
                    Ok(Some(member_info)) => {
                        Ok(HttpResponse::Ok().json(TeamMemberResponse {
                            success: true,
                            message: "Team member updated successfully".to_string(),
                            member: Some(member_info),
                        }))
                    }
                    Ok(None) => {
                        Ok(HttpResponse::Ok().json(TeamMemberResponse {
                            success: true,
                            message: "Team member updated successfully".to_string(),
                            member: None,
                        }))
                    }
                    Err(e) => {
                        tracing::error!("Failed to get updated member info: {}", e);
                        Ok(HttpResponse::Ok().json(TeamMemberResponse {
                            success: true,
                            message: "Team member updated successfully, but failed to get updated info".to_string(),
                            member: None,
                        }))
                    }
                }
            } else {
                Ok(HttpResponse::NotFound().json(TeamMemberResponse {
                    success: false,
                    message: "Team member not found".to_string(),
                    member: None,
                }))
            }
        }
        Err(e) => {
            tracing::error!("Failed to update member {} in team {}: {}", target_user_id, team_id, e);
            Ok(HttpResponse::InternalServerError().json(TeamMemberResponse {
                success: false,
                message: "Failed to update team member".to_string(),
                member: None,
            }))
        }
    }
}