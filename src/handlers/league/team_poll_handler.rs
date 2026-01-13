use actix_web::{web, HttpResponse};
use sqlx::PgPool;
use uuid::Uuid;
use chrono::{Utc, Duration};
use std::sync::Arc;

use crate::middleware::auth::Claims;
use crate::models::team::{
    CreatePollRequest, CastVoteRequest, TeamPollInfo,
    TeamPoll, PollType, PollStatus, PollResult, MemberStatus, TeamRole
};
use crate::services::social_events::send_websocket_notification_to_user;

/// Create a new poll to remove a team member
pub async fn create_poll(
    request: web::Json<CreatePollRequest>,
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<redis::Client>>,
    team_id: web::Path<Uuid>,
    claims: web::ReqData<Claims>,
) -> HttpResponse {
    let Some(user_id) = claims.user_id() else {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "message": "Invalid user ID in token"
        }));
    };

    // Validate request
    if let Err(e) = request.validate() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "message": e
        }));
    }

    let team_id = team_id.into_inner();
    let target_user_id = request.target_user_id;

    // Check if creator is a member of the team
    let creator_membership = match sqlx::query!(
        r#"
        SELECT role as "role: TeamRole", status as "status: MemberStatus"
        FROM team_members
        WHERE team_id = $1 AND user_id = $2
        "#,
        team_id,
        user_id
    )
    .fetch_optional(pool.as_ref())
    .await
    {
        Ok(Some(m)) => m,
        Ok(None) => {
            return HttpResponse::Forbidden().json(serde_json::json!({
                "success": false,
                "message": "You are not a member of this team"
            }));
        }
        Err(e) => {
            tracing::error!("Database error checking creator membership: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "message": "Failed to check team membership"
            }));
        }
    };

    // Creator must be an active member
    if creator_membership.status != MemberStatus::Active {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "success": false,
            "message": "Only active members can create polls"
        }));
    }

    // Check if target user is a member of the team
    let target_membership = match sqlx::query!(
        r#"
        SELECT role as "role: TeamRole", status as "status: MemberStatus"
        FROM team_members
        WHERE team_id = $1 AND user_id = $2
        "#,
        team_id,
        target_user_id
    )
    .fetch_optional(pool.as_ref())
    .await
    {
        Ok(Some(m)) => m,
        Ok(None) => {
            return HttpResponse::BadRequest().json(serde_json::json!({
                "success": false,
                "message": "Target user is not a member of this team"
            }));
        }
        Err(e) => {
            tracing::error!("Database error checking target membership: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "message": "Failed to check target user membership"
            }));
        }
    };

    // Cannot remove owner/captain
    if target_membership.role == TeamRole::Owner {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "success": false,
            "message": "Team captains cannot be removed"
        }));
    }

    // Check if there's already an active poll for this user
    let existing_poll = sqlx::query!(
        "SELECT id FROM team_polls WHERE team_id = $1 AND target_user_id = $2 AND status = 'active'",
        team_id,
        target_user_id
    )
    .fetch_optional(pool.as_ref())
    .await;

    if let Ok(Some(_)) = existing_poll {
        return HttpResponse::Conflict().json(serde_json::json!({
            "success": false,
            "message": "There is already an active poll for this member"
        }));
    }

    // Create the poll (expires in 24 hours)
    let expires_at = Utc::now() + Duration::hours(24);
    let poll_id = Uuid::new_v4();

    let result = sqlx::query!(
        r#"
        INSERT INTO team_polls (id, team_id, poll_type, target_user_id, created_by, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
        poll_id,
        team_id,
        request.poll_type.to_string(),
        target_user_id,
        user_id,
        expires_at
    )
    .execute(pool.as_ref())
    .await;

    if let Err(e) = result {
        tracing::error!("Database error creating poll: {}", e);
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "message": "Failed to create poll"
        }));
    }

    // Get the created poll with full information
    let poll_info = match get_poll_info(&pool, poll_id).await {
        Ok(info) => info,
        Err(e) => {
            tracing::error!("Failed to fetch created poll info: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "message": "Poll created but failed to fetch details"
            }));
        }
    };

    // Notify all active team members (except creator)
    let team_members = sqlx::query!(
        r#"
        SELECT user_id
        FROM team_members
        WHERE team_id = $1 AND status = 'active' AND user_id != $2
        "#,
        team_id,
        user_id
    )
    .fetch_all(pool.as_ref())
    .await
    .unwrap_or_default();

    let notification_message = format!(
        "A poll has been created to remove {} from {}",
        poll_info.target_username,
        poll_info.team_name
    );

    for member in team_members {
        // Create database notification
        let notification_result = sqlx::query!(
            r#"
            INSERT INTO notifications (recipient_id, actor_id, notification_type, entity_type, entity_id, message)
            VALUES ($1, $2, 'team_poll_created', 'poll', $3, $4)
            RETURNING id
            "#,
            member.user_id,
            user_id,
            poll_id,
            &notification_message
        )
        .fetch_one(pool.as_ref())
        .await;

        if let Ok(notification_row) = notification_result {
            // Send WebSocket notification
            match send_websocket_notification_to_user(
                &redis_client,
                member.user_id,
                notification_row.id,
                "Team Vote".to_string(), // Anonymous sender
                "team_poll_created".to_string(),
                notification_message.clone(),
            ).await {
                Ok(_) => {
                    tracing::info!("✅ Sent poll notification to user {}", member.user_id);
                },
                Err(e) => {
                    tracing::error!("❌ Failed to send poll notification to user {}: {}", member.user_id, e);
                }
            }
        } else {
            tracing::error!("❌ Failed to create notification in database for user {}", member.user_id);
        }
    }

    HttpResponse::Created().json(serde_json::json!({
        "success": true,
        "message": "Poll created successfully",
        "poll": poll_info.to_anonymous(user_id)
    }))
}

/// Cast a vote on a poll
pub async fn cast_vote(
    request: web::Json<CastVoteRequest>,
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<redis::Client>>,
    path: web::Path<(Uuid, Uuid)>,
    claims: web::ReqData<Claims>,
) -> HttpResponse {
    let Some(user_id) = claims.user_id() else {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "message": "Invalid user ID in token"
        }));
    };

    let (team_id, poll_id) = path.into_inner();

    // Get the poll
    let poll = match sqlx::query_as!(
        TeamPoll,
        r#"
        SELECT
            id, team_id, poll_type as "poll_type: PollType", target_user_id, created_by,
            created_at, expires_at, status as "status: PollStatus",
            result as "result: PollResult", executed_at
        FROM team_polls
        WHERE id = $1 AND team_id = $2
        "#,
        poll_id,
        team_id
    )
    .fetch_optional(pool.as_ref())
    .await
    {
        Ok(Some(p)) => p,
        Ok(None) => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "message": "Poll not found"
            }));
        }
        Err(e) => {
            tracing::error!("Database error fetching poll: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "message": "Failed to fetch poll"
            }));
        }
    };

    // Check poll is still active
    if poll.status != PollStatus::Active {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "message": "This poll is no longer active"
        }));
    }

    // Check poll hasn't expired
    if poll.expires_at < Utc::now() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "message": "This poll has expired"
        }));
    }

    // Check if user is an active member of the team
    let membership = match sqlx::query!(
        r#"
        SELECT role as "role: TeamRole", status as "status: MemberStatus"
        FROM team_members
        WHERE team_id = $1 AND user_id = $2
        "#,
        team_id,
        user_id
    )
    .fetch_optional(pool.as_ref())
    .await
    {
        Ok(Some(m)) => m,
        Ok(None) => {
            return HttpResponse::Forbidden().json(serde_json::json!({
                "success": false,
                "message": "You are not a member of this team"
            }));
        }
        Err(e) => {
            tracing::error!("Database error checking membership: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "message": "Failed to check team membership"
            }));
        }
    };

    // Only active members can vote
    if membership.status != MemberStatus::Active {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "success": false,
            "message": "Only active members can vote on polls"
        }));
    }

    // Check if user has already voted
    let existing_vote = sqlx::query!(
        "SELECT id FROM poll_votes WHERE poll_id = $1 AND user_id = $2",
        poll_id,
        user_id
    )
    .fetch_optional(pool.as_ref())
    .await;

    if let Ok(Some(_)) = existing_vote {
        return HttpResponse::Conflict().json(serde_json::json!({
            "success": false,
            "message": "You have already voted on this poll"
        }));
    }

    // Insert vote
    let vote_id = Uuid::new_v4();
    let result = sqlx::query!(
        r#"
        INSERT INTO poll_votes (id, poll_id, user_id, vote)
        VALUES ($1, $2, $3, $4)
        "#,
        vote_id,
        poll_id,
        user_id,
        request.vote.to_string()
    )
    .execute(pool.as_ref())
    .await;

    if let Err(e) = result {
        tracing::error!("Database error casting vote: {}", e);
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "message": "Failed to cast vote"
        }));
    }

    // Check if we have enough votes to make a decision
    if let Err(e) = check_and_complete_poll(&pool, &redis_client, poll_id, team_id).await {
        tracing::error!("Error checking/completing poll: {}", e);
        // Don't fail the vote if consensus check fails, just log it
    }

    // Get updated poll info
    let poll_info = match get_poll_info(&pool, poll_id).await {
        Ok(info) => info,
        Err(e) => {
            tracing::error!("Failed to fetch poll info after vote: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "message": "Vote recorded but failed to fetch updated poll"
            }));
        }
    };

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "Vote cast successfully",
        "poll": poll_info.to_anonymous(user_id)
    }))
}

/// Get active polls for a team
pub async fn get_team_polls(
    pool: web::Data<PgPool>,
    team_id: web::Path<Uuid>,
    claims: web::ReqData<Claims>,
) -> HttpResponse {
    let Some(user_id) = claims.user_id() else {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "message": "Invalid user ID in token"
        }));
    };

    let team_id = team_id.into_inner();

    // Check if user is a member of the team
    let is_member = sqlx::query!(
        "SELECT id FROM team_members WHERE team_id = $1 AND user_id = $2",
        team_id,
        user_id
    )
    .fetch_optional(pool.as_ref())
    .await;

    if let Ok(None) = is_member {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "success": false,
            "message": "You are not a member of this team"
        }));
    }

    // Get all polls for the team (active and completed)
    let poll_ids = match sqlx::query!(
        "SELECT id FROM team_polls WHERE team_id = $1 ORDER BY created_at DESC",
        team_id
    )
    .fetch_all(pool.as_ref())
    .await
    {
        Ok(polls) => polls,
        Err(e) => {
            tracing::error!("Database error fetching polls: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "message": "Failed to fetch polls"
            }));
        }
    };

    let mut polls = Vec::new();
    for poll in poll_ids {
        if let Ok(poll_info) = get_poll_info(&pool, poll.id).await {
            polls.push(poll_info.to_anonymous(user_id));
        }
    }

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "polls": polls
    }))
}

/// Helper function to get poll information with vote counts
async fn get_poll_info(pool: &PgPool, poll_id: Uuid) -> Result<TeamPollInfo, sqlx::Error> {
    let poll_data = sqlx::query!(
        r#"
        SELECT
            tp.id, tp.team_id, tp.poll_type, tp.target_user_id, tp.created_by,
            tp.created_at, tp.expires_at, tp.status, tp.result, tp.executed_at,
            t.team_name,
            target_user.username as target_username,
            creator.username as creator_username
        FROM team_polls tp
        JOIN teams t ON tp.team_id = t.id
        JOIN users target_user ON tp.target_user_id = target_user.id
        JOIN users creator ON tp.created_by = creator.id
        WHERE tp.id = $1
        "#,
        poll_id
    )
    .fetch_one(pool)
    .await?;

    // Count votes
    let vote_counts = sqlx::query!(
        r#"
        SELECT
            vote,
            COUNT(*) as count
        FROM poll_votes
        WHERE poll_id = $1
        GROUP BY vote
        "#,
        poll_id
    )
    .fetch_all(pool)
    .await?;

    let mut votes_for = 0;
    let mut votes_against = 0;

    for vc in vote_counts {
        match vc.vote.as_str() {
            "for" => votes_for = vc.count.unwrap_or(0) as i32,
            "against" => votes_against = vc.count.unwrap_or(0) as i32,
            _ => {}
        }
    }

    // Count total eligible voters (all active members including target)
    let eligible_voters = sqlx::query!(
        r#"
        SELECT COUNT(*) as count
        FROM team_members
        WHERE team_id = $1 AND status = 'active'
        "#,
        poll_data.team_id
    )
    .fetch_one(pool)
    .await?;

    Ok(TeamPollInfo {
        id: poll_data.id,
        team_id: poll_data.team_id,
        team_name: poll_data.team_name,
        poll_type: poll_data.poll_type.parse().unwrap_or(PollType::MemberRemoval),
        target_user_id: poll_data.target_user_id,
        target_username: poll_data.target_username,
        created_by: poll_data.created_by,
        created_by_username: poll_data.creator_username,
        created_at: poll_data.created_at,
        expires_at: poll_data.expires_at,
        status: poll_data.status.parse().unwrap_or(PollStatus::Active),
        result: poll_data.result.and_then(|r| r.parse().ok()),
        executed_at: poll_data.executed_at,
        votes_for,
        votes_against,
        total_eligible_voters: eligible_voters.count.unwrap_or(0) as i32,
    })
}

/// Delete a poll (only by creator, and only if active)
pub async fn delete_poll(
    pool: web::Data<PgPool>,
    path: web::Path<(Uuid, Uuid)>,
    claims: web::ReqData<Claims>,
) -> HttpResponse {
    let Some(user_id) = claims.user_id() else {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "message": "Invalid user ID in token"
        }));
    };

    let (team_id, poll_id) = path.into_inner();

    // Get the poll
    let poll = match sqlx::query_as!(
        TeamPoll,
        r#"
        SELECT
            id, team_id, poll_type as "poll_type: PollType", target_user_id, created_by,
            created_at, expires_at, status as "status: PollStatus",
            result as "result: PollResult", executed_at
        FROM team_polls
        WHERE id = $1 AND team_id = $2
        "#,
        poll_id,
        team_id
    )
    .fetch_optional(pool.as_ref())
    .await
    {
        Ok(Some(p)) => p,
        Ok(None) => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "success": false,
                "message": "Poll not found"
            }));
        }
        Err(e) => {
            tracing::error!("Database error fetching poll: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "message": "Failed to fetch poll"
            }));
        }
    };

    // Only the creator can delete the poll
    if poll.created_by != user_id {
        return HttpResponse::Forbidden().json(serde_json::json!({
            "success": false,
            "message": "Only the poll creator can delete this poll"
        }));
    }

    // Can only delete active polls
    if poll.status != PollStatus::Active {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "success": false,
            "message": "Only active polls can be deleted"
        }));
    }

    // Delete associated votes first (foreign key constraint)
    let delete_votes_result = sqlx::query!(
        "DELETE FROM poll_votes WHERE poll_id = $1",
        poll_id
    )
    .execute(pool.as_ref())
    .await;

    if let Err(e) = delete_votes_result {
        tracing::error!("Database error deleting poll votes: {}", e);
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "message": "Failed to delete poll votes"
        }));
    }

    // Delete the poll
    let delete_result = sqlx::query!(
        "DELETE FROM team_polls WHERE id = $1",
        poll_id
    )
    .execute(pool.as_ref())
    .await;

    if let Err(e) = delete_result {
        tracing::error!("Database error deleting poll: {}", e);
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "message": "Failed to delete poll"
        }));
    }

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "Poll deleted successfully"
    }))
}

/// Check if a poll has reached consensus and complete it if so
async fn check_and_complete_poll(
    pool: &PgPool,
    redis_client: &web::Data<Arc<redis::Client>>,
    poll_id: Uuid,
    team_id: Uuid,
) -> Result<(), String> {
    let poll_info = get_poll_info(pool, poll_id).await
        .map_err(|e| format!("Failed to get poll info: {e}"))?;

    let total_votes = poll_info.votes_for + poll_info.votes_against;
    let votes_remaining = poll_info.total_eligible_voters - total_votes;
    let votes_needed_to_approve = (poll_info.total_eligible_voters / 2) + 1; // Need majority

    // Check if consensus has been reached (even if not everyone has voted)
    let consensus_reached =
        // Approval is certain: enough votes for approval already
        poll_info.votes_for >= votes_needed_to_approve ||
        // Rejection is certain: even if all remaining votes are "for", won't reach majority
        poll_info.votes_for + votes_remaining < votes_needed_to_approve ||
        // All eligible voters have voted
        total_votes >= poll_info.total_eligible_voters;

    if !consensus_reached {
        // Consensus not yet reached, poll remains active
        return Ok(());
    }

    // Determine result - need majority (more than 50%) to approve
    let result = if poll_info.votes_for >= votes_needed_to_approve {
        PollResult::Approved
    } else {
        PollResult::Rejected
    };

    // Update poll status
    sqlx::query!(
        "UPDATE team_polls SET status = 'completed', result = $1, executed_at = NOW() WHERE id = $2",
        result.to_string(),
        poll_id
    )
    .execute(pool)
    .await
    .map_err(|e| format!("Failed to update poll status: {e}"))?;

    // If approved, remove the member from the team and add back to player pool
    if result == PollResult::Approved {
        let removal_result = sqlx::query!(
            "DELETE FROM team_members WHERE team_id = $1 AND user_id = $2",
            poll_info.team_id,
            poll_info.target_user_id
        )
        .execute(pool)
        .await;

        if let Err(e) = removal_result {
            tracing::error!("Failed to remove member after poll approval: {}", e);
        } else {
            // Add user back to player pool
            match sqlx::query!(
                r#"
                INSERT INTO player_pool (user_id)
                VALUES ($1)
                ON CONFLICT (user_id) DO NOTHING
                "#,
                poll_info.target_user_id
            )
            .execute(pool)
            .await {
                Ok(_) => {
                    tracing::info!("✅ Added user {} back to player pool after team removal", poll_info.target_user_id);
                },
                Err(e) => {
                    tracing::error!("❌ Failed to add user {} to player pool: {}", poll_info.target_user_id, e);
                }
            }

            // Notify the removed user about team removal
            let notification_message = format!("You have been removed from team {} by a member vote", poll_info.team_name);

            let notification_result = sqlx::query!(
                r#"
                INSERT INTO notifications (recipient_id, actor_id, notification_type, entity_type, entity_id, message)
                VALUES ($1, $1, 'removed_from_team', 'poll', $2, $3)
                RETURNING id
                "#,
                poll_info.target_user_id,
                poll_id,
                &notification_message
            )
            .fetch_one(pool)
            .await;

            if let Ok(notification_row) = notification_result {
                let _ = send_websocket_notification_to_user(
                    redis_client,
                    poll_info.target_user_id,
                    notification_row.id,
                    "Team Vote".to_string(),
                    "removed_from_team".to_string(),
                    notification_message,
                ).await;
            }

            // Also notify about becoming a free agent
            let free_agent_message = "You are now a free agent";
            match sqlx::query!(
                r#"
                INSERT INTO notifications (recipient_id, actor_id, notification_type, entity_type, entity_id, message)
                VALUES ($1, $1, 'player_pool_event', 'player_pool', $1, $2)
                RETURNING id
                "#,
                poll_info.target_user_id,
                free_agent_message
            )
            .fetch_one(pool)
            .await {
                Ok(notif_row) => {
                    let _ = send_websocket_notification_to_user(
                        redis_client,
                        poll_info.target_user_id,
                        notif_row.id,
                        "Player Pool".to_string(),
                        "player_pool_event".to_string(),
                        free_agent_message.to_string(),
                    ).await;
                    tracing::info!("📬 Sent free agent notification to user {}", poll_info.target_user_id);
                },
                Err(e) => {
                    tracing::error!("❌ Failed to create free agent notification for user {}: {}", poll_info.target_user_id, e);
                }
            }

            tracing::info!("✅ Poll {} completed: removed user {} from team {} and added to player pool",
                poll_id, poll_info.target_username, poll_info.team_name);
        }
    }

    // Notify all team members of the poll completion
    let team_members = sqlx::query!(
        "SELECT user_id FROM team_members WHERE team_id = $1 AND status = 'active'",
        team_id
    )
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch team members: {e}"))?;

    let notification_message = if result == PollResult::Approved {
        format!("Poll completed: {} has been removed from {}", poll_info.target_username, poll_info.team_name)
    } else {
        format!("Poll completed: {} will remain in {}", poll_info.target_username, poll_info.team_name)
    };

    for member in team_members {
        let notification_result = sqlx::query!(
            r#"
            INSERT INTO notifications (recipient_id, actor_id, notification_type, entity_type, entity_id, message)
            VALUES ($1, $1, 'team_poll_completed', 'poll', $2, $3)
            RETURNING id
            "#,
            member.user_id,
            poll_id,
            &notification_message
        )
        .fetch_one(pool)
        .await;

        if let Ok(notification_row) = notification_result {
            let _ = send_websocket_notification_to_user(
                redis_client,
                member.user_id,
                notification_row.id,
                "Team Vote".to_string(),
                "team_poll_completed".to_string(),
                notification_message.clone(),
            ).await;
        }
    }

    Ok(())
}
