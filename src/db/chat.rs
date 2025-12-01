use sqlx::{PgPool, Row};
use uuid::Uuid;
use chrono::Utc;

use crate::models::chat::{TeamChatMessage, TeamChatMessageInfo};
use crate::models::team::MemberStatus;

/// Create a new chat message
pub async fn create_chat_message(
    pool: &PgPool,
    team_id: Uuid,
    user_id: Uuid,
    message: &str,
) -> Result<TeamChatMessage, sqlx::Error> {
    let chat_message = sqlx::query_as::<_, TeamChatMessage>(
        r#"
        INSERT INTO team_chat_messages (team_id, user_id, message)
        VALUES ($1, $2, $3)
        RETURNING id, team_id, user_id, message, created_at, edited_at, deleted_at
        "#,
    )
    .bind(team_id)
    .bind(user_id)
    .bind(message)
    .fetch_one(pool)
    .await?;

    Ok(chat_message)
}

/// Get chat message by ID
pub async fn get_chat_message(
    pool: &PgPool,
    message_id: Uuid,
) -> Result<TeamChatMessage, sqlx::Error> {
    sqlx::query_as::<_, TeamChatMessage>(
        r#"
        SELECT id, team_id, user_id, message, created_at, edited_at, deleted_at
        FROM team_chat_messages
        WHERE id = $1
        "#,
    )
    .bind(message_id)
    .fetch_one(pool)
    .await
}

/// Get chat message with user info
pub async fn get_chat_message_with_user(
    pool: &PgPool,
    message_id: Uuid,
) -> Result<TeamChatMessageInfo, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT
            tcm.id,
            tcm.team_id,
            tcm.user_id,
            u.username,
            tcm.message,
            tcm.created_at,
            tcm.edited_at,
            tcm.deleted_at
        FROM team_chat_messages tcm
        INNER JOIN users u ON u.id = tcm.user_id
        WHERE tcm.id = $1
        "#,
    )
    .bind(message_id)
    .fetch_one(pool)
    .await?;

    Ok(TeamChatMessageInfo {
        id: row.get("id"),
        team_id: row.get("team_id"),
        user_id: row.get("user_id"),
        username: row.get("username"),
        message: row.get("message"),
        created_at: row.get("created_at"),
        edited_at: row.get("edited_at"),
        deleted_at: row.get("deleted_at"),
    })
}

/// Get chat history for a team with pagination
pub async fn get_team_chat_history(
    pool: &PgPool,
    team_id: Uuid,
    limit: i64,
    before_message_id: Option<Uuid>,
) -> Result<Vec<TeamChatMessageInfo>, sqlx::Error> {
    let query = if let Some(before_id) = before_message_id {
        // Get messages before a specific message (for pagination)
        sqlx::query(
            r#"
            SELECT
                tcm.id,
                tcm.team_id,
                tcm.user_id,
                u.username,
                tcm.message,
                tcm.created_at,
                tcm.edited_at,
                tcm.deleted_at
            FROM team_chat_messages tcm
            INNER JOIN users u ON u.id = tcm.user_id
            WHERE tcm.team_id = $1
                AND tcm.deleted_at IS NULL
                AND tcm.created_at < (SELECT created_at FROM team_chat_messages WHERE id = $2)
            ORDER BY tcm.created_at DESC
            LIMIT $3
            "#,
        )
        .bind(team_id)
        .bind(before_id)
        .bind(limit)
    } else {
        // Get most recent messages
        sqlx::query(
            r#"
            SELECT
                tcm.id,
                tcm.team_id,
                tcm.user_id,
                u.username,
                tcm.message,
                tcm.created_at,
                tcm.edited_at,
                tcm.deleted_at
            FROM team_chat_messages tcm
            INNER JOIN users u ON u.id = tcm.user_id
            WHERE tcm.team_id = $1 AND tcm.deleted_at IS NULL
            ORDER BY tcm.created_at DESC
            LIMIT $2
            "#,
        )
        .bind(team_id)
        .bind(limit)
    };

    let rows = query.fetch_all(pool).await?;

    let mut messages = Vec::new();
    for row in rows {
        messages.push(TeamChatMessageInfo {
            id: row.get("id"),
            team_id: row.get("team_id"),
            user_id: row.get("user_id"),
            username: row.get("username"),
            message: row.get("message"),
            created_at: row.get("created_at"),
            edited_at: row.get("edited_at"),
            deleted_at: row.get("deleted_at"),
        });
    }

    // Reverse to get chronological order (oldest first)
    messages.reverse();

    Ok(messages)
}

/// Get total message count for a team
pub async fn get_team_message_count(
    pool: &PgPool,
    team_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) as count
        FROM team_chat_messages
        WHERE team_id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(team_id)
    .fetch_one(pool)
    .await?;

    Ok(row.get("count"))
}

/// Edit a chat message
pub async fn edit_chat_message(
    pool: &PgPool,
    message_id: Uuid,
    user_id: Uuid,
    new_message: &str,
) -> Result<TeamChatMessage, sqlx::Error> {
    sqlx::query_as::<_, TeamChatMessage>(
        r#"
        UPDATE team_chat_messages
        SET message = $1, edited_at = $2
        WHERE id = $3 AND user_id = $4 AND deleted_at IS NULL
        RETURNING id, team_id, user_id, message, created_at, edited_at, deleted_at
        "#,
    )
    .bind(new_message)
    .bind(Utc::now())
    .bind(message_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
}

/// Soft delete a chat message
pub async fn delete_chat_message(
    pool: &PgPool,
    message_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE team_chat_messages
        SET deleted_at = $1
        WHERE id = $2 AND user_id = $3 AND deleted_at IS NULL
        "#,
    )
    .bind(Utc::now())
    .bind(message_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Admin delete a chat message (for team admins/owners)
pub async fn admin_delete_chat_message(
    pool: &PgPool,
    message_id: Uuid,
    team_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE team_chat_messages
        SET deleted_at = $1
        WHERE id = $2 AND team_id = $3 AND deleted_at IS NULL
        "#,
    )
    .bind(Utc::now())
    .bind(message_id)
    .bind(team_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

/// Check if user is an active member of a team
pub async fn is_active_team_member(
    pool: &PgPool,
    user_id: Uuid,
    team_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) as count
        FROM team_members
        WHERE user_id = $1 AND team_id = $2 AND status = $3
        "#,
    )
    .bind(user_id)
    .bind(team_id)
    .bind(MemberStatus::Active.to_string())
    .fetch_one(pool)
    .await?;

    let count: i64 = row.get("count");
    Ok(count > 0)
}

/// Check if user is team admin or owner
pub async fn is_team_admin_or_owner(
    pool: &PgPool,
    user_id: Uuid,
    team_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) as count
        FROM team_members
        WHERE user_id = $1
            AND team_id = $2
            AND status = 'active'
            AND (role = 'admin' OR role = 'owner')
        "#,
    )
    .bind(user_id)
    .bind(team_id)
    .fetch_one(pool)
    .await?;

    let count: i64 = row.get("count");
    Ok(count > 0)
}

/// Get all active team member user IDs
pub async fn get_active_team_member_ids(
    pool: &PgPool,
    team_id: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT user_id
        FROM team_members
        WHERE team_id = $1 AND status = $2
        "#,
    )
    .bind(team_id)
    .bind(MemberStatus::Active.to_string())
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(|row| row.get("user_id")).collect())
}

/// Get all team IDs for a user
pub async fn get_user_team_ids(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<Uuid>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT team_id
        FROM team_members
        WHERE user_id = $1 AND status = $2
        "#,
    )
    .bind(user_id)
    .bind(MemberStatus::Active.to_string())
    .fetch_all(pool)
    .await?;

    Ok(rows.iter().map(|row| row.get("team_id")).collect())
}
