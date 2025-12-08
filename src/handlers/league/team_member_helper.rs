use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::models::team::*;

pub async fn add_member(
    team_id: Uuid,
    member: &TeamMemberRequest,
    pool: &PgPool,
    requester_role: &TeamRole,
) -> Result<TeamMemberInfo, sqlx::Error> {
    
    // Find the target user
    let target_user_id = match find_user_by_request(member, pool).await {
        Ok(Some(user_id)) => user_id,
        Ok(None) => {
            tracing::error!("User not found");
            return Err(sqlx::Error::TypeNotFound { type_name: "User".to_string() });
        }
        Err(e) => {
            tracing::error!("Failed to find target user: {}", e);
            return Err(e);
        }
    };

    // Check if user is already a member
    match check_team_member_role(&team_id, &target_user_id, pool).await {
        Ok(Some(_)) => {
            tracing::error!("User is already a member of this team");
            return Err(sqlx::Error::TypeNotFound { type_name: "User".to_string() });
        }
        Ok(None) => {
            tracing::info!("User is not a member of this team - proceeding");
        }
        Err(e) => {
            tracing::error!("Failed to check existing membership: {}", e);
            return Err(e);
        }
    }

    // Determine the role for the new member
    let member_role = member.role.clone().unwrap_or(TeamRole::Member);

    // Only owners can add other owners
    if member_role == TeamRole::Owner && requester_role != &TeamRole::Owner {
        tracing::error!("Only team owners can add other owners");
        return Err(sqlx::Error::TypeNotFound { type_name: "User".to_string() });
    }

    // Add the user to the team
    let member_id = Uuid::new_v4();
    let now = Utc::now();

    match sqlx::query!(
        r#"
        INSERT INTO team_members (id, team_id, user_id, role, status, joined_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        member_id,
        team_id,
        target_user_id,
        member_role.to_string(),
        MemberStatus::Active.to_string(),
        now,
        now
    )
    .execute(pool)
    .await
    {
        Ok(_) => {
            tracing::info!("Successfully added user {} to team {} as {}", 
                target_user_id, team_id, member_role);

            // Get the member info for response
            match get_team_member_info(&team_id, &target_user_id, pool).await {
                Ok(Some(member_info)) => {
                    Ok(member_info)
                }
                Ok(None) => {
                    tracing::error!("Failed to get member info after adding");
                    Err(sqlx::Error::TypeNotFound { type_name: "TeamMemberInfo".to_string() })
                }
                Err(e) => {
                    tracing::error!("Failed to get member info after adding");
                    Err(e)
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to add user {} to team {}", target_user_id, team_id);
            Err(e)
        }
    }
}

async fn find_user_by_request(request: &TeamMemberRequest, pool: &PgPool) -> Result<Option<Uuid>, sqlx::Error> {
    if let Some(user_id) = request.user_id {
        // Check if user exists by ID
        let result = sqlx::query!(
            "SELECT id FROM users WHERE id = $1",
            user_id
        )
        .fetch_optional(pool)
        .await?;
        
        return Ok(result.map(|row| row.id));
    }

    if let Some(username) = &request.username {
        let result = sqlx::query!(
            "SELECT id FROM users WHERE username = $1",
            username
        )
        .fetch_optional(pool)
        .await?;
        
        return Ok(result.map(|row| row.id));
    }

    if let Some(email) = &request.email {
        let result = sqlx::query!(
            "SELECT id FROM users WHERE email = $1",
            email
        )
        .fetch_optional(pool)
        .await?;
        
        return Ok(result.map(|row| row.id));
    }

    Ok(None)
}

pub async fn check_team_member_role(team_id: &Uuid, user_id: &Uuid, pool: &PgPool) -> Result<Option<TeamRole>, sqlx::Error> {
    let result = sqlx::query!(
        "SELECT role FROM team_members WHERE team_id = $1 AND user_id = $2 AND status = 'active'",
        team_id,
        user_id
    )
    .fetch_optional(pool)
    .await?;

    match result {
        Some(row) => Ok(Some(row.role.parse().map_err(|_| sqlx::Error::TypeNotFound { type_name: "TeamRole".to_string() })?)),
        None => Ok(None),
    }
}

pub async fn get_team_member_info(team_id: &Uuid, user_id: &Uuid, pool: &PgPool) -> Result<Option<TeamMemberInfo>, sqlx::Error> {
    sqlx::query_as!(
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
        WHERE tm.team_id = $1 AND tm.user_id = $2
        "#,
        team_id,
        user_id
    )
    .fetch_optional(pool)
    .await
}

pub async fn get_team_info(team_id: &Uuid, pool: &PgPool) -> Result<Option<TeamInfo>, sqlx::Error> {
    sqlx::query_as!(
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
    .fetch_optional(pool)
    .await
}

pub async fn count_team_owners(team_id: &Uuid, pool: &PgPool) -> Result<i64, sqlx::Error> {
    let result = sqlx::query!(
        "SELECT COUNT(*) as count FROM team_members WHERE team_id = $1 AND role = 'owner' AND status = 'active'",
        team_id
    )
    .fetch_one(pool)
    .await?;

    Ok(result.count.unwrap_or(0))
}

/// Remove a user from a team and add them back to player pool if they're active
pub async fn remove_member_and_return_to_pool(
    team_id: &Uuid,
    user_id: &Uuid,
    pool: &PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get user status before removal
    let user_info = sqlx::query!(
        r#"
        SELECT status as "status: crate::models::user::UserStatus"
        FROM users
        WHERE id = $1
        "#,
        user_id
    )
    .fetch_one(pool)
    .await?;

    // Remove the member
    let result = sqlx::query!(
        "DELETE FROM team_members WHERE team_id = $1 AND user_id = $2",
        team_id,
        user_id
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err("User not found in team".into());
    }

    tracing::info!("Successfully removed user {} from team {}", user_id, team_id);

    // If user is still active, add them back to player pool
    if user_info.status == crate::models::user::UserStatus::Active {
        let add_result = sqlx::query!(
            r#"
            INSERT INTO player_pool (user_id, last_active_at)
            VALUES ($1, NOW())
            ON CONFLICT (user_id)
            DO UPDATE SET
                last_active_at = NOW(),
                updated_at = NOW()
            "#,
            user_id
        )
        .execute(pool)
        .await;

        match add_result {
            Ok(_) => {
                tracing::info!("Added user {} back to player pool after team removal", user_id);
            }
            Err(e) => {
                tracing::warn!("Failed to add user back to player pool: {}", e);
                // Don't fail the removal operation
            }
        }
    }

    Ok(())
}

/// Remove a user from player pool after joining a team
pub async fn remove_from_player_pool(
    user_id: &Uuid,
    pool: &PgPool,
) -> Result<(), Box<dyn std::error::Error>> {
    match sqlx::query!(
        "DELETE FROM player_pool WHERE user_id = $1",
        user_id
    )
    .execute(pool)
    .await
    {
        Ok(_) => {
            tracing::info!("Removed user {} from player pool after joining team", user_id);
            Ok(())
        }
        Err(e) => {
            tracing::warn!("Failed to remove user from player pool: {}", e);
            // Don't fail the operation if pool removal fails
            Ok(())
        }
    }
}