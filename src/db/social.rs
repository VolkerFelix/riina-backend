use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::models::social::{
    CommentListResponse, WorkoutComment, WorkoutCommentWithUser, WorkoutReaction,
    WorkoutReactionWithUser, WorkoutReactionSummary,
    CommentReaction, CommentReactionWithUser, CommentReactionSummary,
    NotificationWithUser, NotificationListResponse,
};

pub async fn create_reaction(
    pool: &PgPool,
    user_id: Uuid,
    workout_id: Uuid,
    reaction_type: &str,
) -> Result<WorkoutReaction, sqlx::Error> {
    let reaction = sqlx::query_as::<_, WorkoutReaction>(
        r#"
        INSERT INTO post_reactions (user_id, workout_id, reaction_type)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id, workout_id)
        DO UPDATE SET reaction_type = $3, created_at = NOW()
        RETURNING id, user_id, workout_id, reaction_type, created_at
        "#,
    )
    .bind(user_id)
    .bind(workout_id)
    .bind(reaction_type)
    .fetch_one(pool)
    .await?;

    Ok(reaction)
}

pub async fn delete_reaction(
    pool: &PgPool,
    user_id: Uuid,
    workout_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        DELETE FROM post_reactions
        WHERE user_id = $1 AND workout_id = $2
        "#,
    )
    .bind(user_id)
    .bind(workout_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn get_workout_reactions(
    pool: &PgPool,
    workout_id: Uuid,
    current_user_id: Option<Uuid>,
) -> Result<WorkoutReactionSummary, sqlx::Error> {
    // Get count of fire reactions and whether current user reacted
    let result = sqlx::query(
        r#"
        SELECT
            COALESCE(COUNT(wr.id), 0) as fire_count,
            COALESCE(BOOL_OR(wr.user_id = $2), false) as user_reacted
        FROM post_reactions wr
        WHERE wr.workout_id = $1 AND wr.reaction_type = 'fire'
        "#,
    )
    .bind(workout_id)
    .bind(current_user_id)
    .fetch_one(pool)
    .await?;

    Ok(WorkoutReactionSummary {
        workout_id,
        fire_count: result.get("fire_count"),
        user_reacted: result.get("user_reacted"),
    })
}

pub async fn get_reaction_users(
    pool: &PgPool,
    workout_id: Uuid,
    reaction_type: Option<&str>,
) -> Result<Vec<WorkoutReactionWithUser>, sqlx::Error> {
    let query = if let Some(reaction_type) = reaction_type {
        sqlx::query(
            r#"
            SELECT
                wr.id,
                wr.user_id,
                u.username,
                wr.reaction_type,
                wr.created_at
            FROM post_reactions wr
            INNER JOIN users u ON u.id = wr.user_id
            WHERE wr.workout_id = $1 AND wr.reaction_type = $2
            ORDER BY wr.created_at DESC
            "#,
        )
        .bind(workout_id)
        .bind(reaction_type)
    } else {
        sqlx::query(
            r#"
            SELECT
                wr.id,
                wr.user_id,
                u.username,
                wr.reaction_type,
                wr.created_at
            FROM post_reactions wr
            INNER JOIN users u ON u.id = wr.user_id
            WHERE wr.workout_id = $1
            ORDER BY wr.created_at DESC
            "#,
        )
        .bind(workout_id)
    };

    let rows = query.fetch_all(pool).await?;
    let reactions = rows
        .into_iter()
        .map(|row| WorkoutReactionWithUser {
            id: row.get("id"),
            user_id: row.get("user_id"),
            username: row.get("username"),
            reaction_type: row.get("reaction_type"),
            created_at: row.get("created_at"),
        })
        .collect();
    Ok(reactions)
}

pub async fn create_comment(
    pool: &PgPool,
    user_id: Uuid,
    workout_id: Uuid,
    content: &str,
    parent_id: Option<Uuid>,
) -> Result<WorkoutComment, sqlx::Error> {
    let comment = sqlx::query_as::<_, WorkoutComment>(
        r#"
        INSERT INTO post_comments (user_id, workout_id, content, parent_id)
        VALUES ($1, $2, $3, $4)
        RETURNING id, user_id, workout_id, parent_id, content, is_edited, created_at, updated_at
        "#,
    )
    .bind(user_id)
    .bind(workout_id)
    .bind(content)
    .bind(parent_id)
    .fetch_one(pool)
    .await?;

    Ok(comment)
}

pub async fn update_comment(
    pool: &PgPool,
    comment_id: Uuid,
    user_id: Uuid,
    content: &str,
) -> Result<Option<WorkoutComment>, sqlx::Error> {
    let comment = sqlx::query_as::<_, WorkoutComment>(
        r#"
        UPDATE post_comments
        SET content = $3
        WHERE id = $1 AND user_id = $2
        RETURNING id, user_id, workout_id, parent_id, content, is_edited, created_at, updated_at
        "#,
    )
    .bind(comment_id)
    .bind(user_id)
    .bind(content)
    .fetch_optional(pool)
    .await?;

    Ok(comment)
}

pub async fn delete_comment(
    pool: &PgPool,
    comment_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        DELETE FROM post_comments
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(comment_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn get_workout_comments(
    pool: &PgPool,
    workout_id: Uuid,
    page: i32,
    per_page: i32,
    current_user_id: Option<Uuid>,
) -> Result<CommentListResponse, sqlx::Error> {
    let offset = (page - 1) * per_page;

    let total_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM post_comments
        WHERE workout_id = $1 AND parent_id IS NULL
        "#,
    )
    .bind(workout_id)
    .fetch_one(pool)
    .await?;

    // Get top-level comments first with reaction data
    let top_level_comments = sqlx::query(
        r#"
        SELECT
            c.id,
            c.user_id,
            u.username,
            c.workout_id,
            c.parent_id,
            c.content,
            c.is_edited,
            c.created_at,
            c.updated_at,
            COALESCE(COUNT(cr.id), 0) as fire_count,
            COALESCE(BOOL_OR(cr.user_id = $4), false) as user_reacted
        FROM post_comments c
        INNER JOIN users u ON u.id = c.user_id
        LEFT JOIN post_comment_reactions cr ON cr.comment_id = c.id AND cr.reaction_type = 'fire'
        WHERE c.workout_id = $1 AND c.parent_id IS NULL
        GROUP BY c.id, c.user_id, u.username, c.workout_id, c.parent_id, c.content, c.is_edited, c.created_at, c.updated_at
        ORDER BY c.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(workout_id)
    .bind(per_page)
    .bind(offset)
    .bind(current_user_id)
    .fetch_all(pool)
    .await?;

    let mut final_comments = Vec::new();

    for row in top_level_comments {
        let mut comment = WorkoutCommentWithUser {
            id: row.get("id"),
            user_id: row.get("user_id"),
            username: row.get("username"),
            workout_id: row.get("workout_id"),
            parent_id: row.get("parent_id"),
            content: row.get("content"),
            is_edited: row.get("is_edited"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            replies: Vec::new(),
            fire_count: row.get("fire_count"),
            user_reacted: row.get("user_reacted"),
        };

        // Get replies for this comment with reaction data
        let replies = sqlx::query(
            r#"
            SELECT
                c.id,
                c.user_id,
                u.username,
                c.workout_id,
                c.parent_id,
                c.content,
                c.is_edited,
                c.created_at,
                c.updated_at,
                COALESCE(COUNT(cr.id), 0) as fire_count,
                COALESCE(BOOL_OR(cr.user_id = $2), false) as user_reacted
            FROM post_comments c
            INNER JOIN users u ON u.id = c.user_id
            LEFT JOIN post_comment_reactions cr ON cr.comment_id = c.id AND cr.reaction_type = 'fire'
            WHERE c.parent_id = $1
            GROUP BY c.id, c.user_id, u.username, c.workout_id, c.parent_id, c.content, c.is_edited, c.created_at, c.updated_at
            ORDER BY c.created_at ASC
            "#,
        )
        .bind(comment.id)
        .bind(current_user_id)
        .fetch_all(pool)
        .await?;

        for reply_row in replies {
            let reply = WorkoutCommentWithUser {
                id: reply_row.get("id"),
                user_id: reply_row.get("user_id"),
                username: reply_row.get("username"),
                workout_id: reply_row.get("workout_id"),
                parent_id: reply_row.get("parent_id"),
                content: reply_row.get("content"),
                is_edited: reply_row.get("is_edited"),
                created_at: reply_row.get("created_at"),
                updated_at: reply_row.get("updated_at"),
                replies: Vec::new(),
                fire_count: reply_row.get("fire_count"),
                user_reacted: reply_row.get("user_reacted"),
            };
            comment.replies.push(reply);
        }

        final_comments.push(comment);
    }

    Ok(CommentListResponse {
        comments: final_comments,
        total_count,
        page,
        per_page,
    })
}

pub async fn get_comment_by_id(
    pool: &PgPool,
    comment_id: Uuid,
) -> Result<Option<WorkoutCommentWithUser>, sqlx::Error> {
    let comment = sqlx::query(
        r#"
        SELECT
            c.id,
            c.user_id,
            u.username,
            c.workout_id,
            c.parent_id,
            c.content,
            c.is_edited,
            c.created_at,
            c.updated_at
        FROM post_comments c
        INNER JOIN users u ON u.id = c.user_id
        WHERE c.id = $1
        "#,
    )
    .bind(comment_id)
    .fetch_optional(pool)
    .await?
    .map(|row| WorkoutCommentWithUser {
        id: row.get("id"),
        user_id: row.get("user_id"),
        username: row.get("username"),
        workout_id: row.get("workout_id"),
        parent_id: row.get("parent_id"),
        content: row.get("content"),
        is_edited: row.get("is_edited"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
        replies: Vec::new(),
        fire_count: 0,
        user_reacted: false,
    });

    Ok(comment)
}

// ============================================================================
// COMMENT REACTION FUNCTIONS
// ============================================================================

pub async fn create_comment_reaction(
    pool: &PgPool,
    user_id: Uuid,
    comment_id: Uuid,
    reaction_type: &str,
) -> Result<CommentReaction, sqlx::Error> {
    let reaction = sqlx::query_as::<_, CommentReaction>(
        r#"
        INSERT INTO post_comment_reactions (user_id, comment_id, reaction_type)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id, comment_id)
        DO UPDATE SET reaction_type = $3, created_at = NOW()
        RETURNING id, user_id, comment_id, reaction_type, created_at
        "#,
    )
    .bind(user_id)
    .bind(comment_id)
    .bind(reaction_type)
    .fetch_one(pool)
    .await?;

    Ok(reaction)
}

pub async fn delete_comment_reaction(
    pool: &PgPool,
    user_id: Uuid,
    comment_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        DELETE FROM comment_reactions
        WHERE user_id = $1 AND comment_id = $2
        "#,
    )
    .bind(user_id)
    .bind(comment_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn get_comment_reactions(
    pool: &PgPool,
    comment_id: Uuid,
    current_user_id: Option<Uuid>,
) -> Result<CommentReactionSummary, sqlx::Error> {
    // Get count of fire reactions and whether current user reacted
    let result = sqlx::query(
        r#"
        SELECT
            COALESCE(COUNT(cr.id), 0) as fire_count,
            COALESCE(BOOL_OR(cr.user_id = $2), false) as user_reacted
        FROM post_comment_reactions cr
        WHERE cr.comment_id = $1 AND cr.reaction_type = 'fire'
        "#,
    )
    .bind(comment_id)
    .bind(current_user_id)
    .fetch_one(pool)
    .await?;

    Ok(CommentReactionSummary {
        comment_id,
        fire_count: result.get("fire_count"),
        user_reacted: result.get("user_reacted"),
    })
}

pub async fn get_comment_reaction_users(
    pool: &PgPool,
    comment_id: Uuid,
    reaction_type: Option<&str>,
) -> Result<Vec<CommentReactionWithUser>, sqlx::Error> {
    let query = if let Some(reaction_type) = reaction_type {
        sqlx::query(
            r#"
            SELECT
                cr.id,
                cr.user_id,
                u.username,
                cr.reaction_type,
                cr.created_at
            FROM post_comment_reactions cr
            INNER JOIN users u ON u.id = cr.user_id
            WHERE cr.comment_id = $1 AND cr.reaction_type = $2
            ORDER BY cr.created_at DESC
            "#,
        )
        .bind(comment_id)
        .bind(reaction_type)
    } else {
        sqlx::query(
            r#"
            SELECT
                cr.id,
                cr.user_id,
                u.username,
                cr.reaction_type,
                cr.created_at
            FROM post_comment_reactions cr
            INNER JOIN users u ON u.id = cr.user_id
            WHERE cr.comment_id = $1
            ORDER BY cr.created_at DESC
            "#,
        )
        .bind(comment_id)
    };

    let rows = query.fetch_all(pool).await?;
    let reactions = rows
        .into_iter()
        .map(|row| CommentReactionWithUser {
            id: row.get("id"),
            user_id: row.get("user_id"),
            username: row.get("username"),
            reaction_type: row.get("reaction_type"),
            created_at: row.get("created_at"),
        })
        .collect();
    Ok(reactions)
}

// ============================================================================
// NOTIFICATION FUNCTIONS
// ============================================================================

pub async fn create_notification(
    pool: &PgPool,
    recipient_id: Uuid,
    actor_id: Uuid,
    notification_type: &str,
    entity_type: &str,
    entity_id: Uuid,
    message: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    // Don't create notification if actor is the recipient
    if actor_id == recipient_id {
        return Ok(None);
    }

    let notification_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO notifications (recipient_id, actor_id, notification_type, entity_type, entity_id, message)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#,
    )
    .bind(recipient_id)
    .bind(actor_id)
    .bind(notification_type)
    .bind(entity_type)
    .bind(entity_id)
    .bind(message)
    .fetch_one(pool)
    .await?;

    Ok(Some(notification_id))
}

pub async fn get_notifications(
    pool: &PgPool,
    user_id: Uuid,
    page: i32,
    per_page: i32,
    unread_only: bool,
) -> Result<NotificationListResponse, sqlx::Error> {
    let offset = (page - 1) * per_page;

    // Get total count
    let total_count: i64 = if unread_only {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM notifications
            WHERE recipient_id = $1 AND read = false
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?
    } else {
        sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM notifications
            WHERE recipient_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_one(pool)
        .await?
    };

    // Get unread count
    let unread_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM notifications
        WHERE recipient_id = $1 AND read = false
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    // Get notifications (user-specific AND broadcast notifications)
    let query_str = if unread_only {
        r#"
        SELECT
            n.id,
            n.recipient_id,
            n.actor_id,
            u.username as actor_username,
            n.notification_type,
            n.entity_type,
            n.entity_id,
            n.message,
            n.read,
            n.created_at
        FROM notifications n
        INNER JOIN users u ON u.id = n.actor_id
        WHERE (n.recipient_id = $1 OR n.recipient_id IS NULL) AND n.read = false
        ORDER BY n.created_at DESC
        LIMIT $2 OFFSET $3
        "#
    } else {
        r#"
        SELECT
            n.id,
            n.recipient_id,
            n.actor_id,
            u.username as actor_username,
            n.notification_type,
            n.entity_type,
            n.entity_id,
            n.message,
            n.read,
            n.created_at
        FROM notifications n
        INNER JOIN users u ON u.id = n.actor_id
        WHERE n.recipient_id = $1 OR n.recipient_id IS NULL
        ORDER BY n.created_at DESC
        LIMIT $2 OFFSET $3
        "#
    };

    let rows = sqlx::query(query_str)
        .bind(user_id)
        .bind(per_page)
        .bind(offset)
        .fetch_all(pool)
        .await?;

    let notifications = rows
        .into_iter()
        .map(|row| NotificationWithUser {
            id: row.get("id"),
            recipient_id: row.get("recipient_id"),
            actor_id: row.get("actor_id"),
            actor_username: row.get("actor_username"),
            notification_type: row.get("notification_type"),
            entity_type: row.get("entity_type"),
            entity_id: row.get("entity_id"),
            message: row.get("message"),
            read: row.get("read"),
            created_at: row.get("created_at"),
        })
        .collect();

    Ok(NotificationListResponse {
        notifications,
        total_count,
        unread_count,
        page,
        per_page,
    })
}

pub async fn mark_notification_read(
    pool: &PgPool,
    notification_id: Uuid,
    user_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE notifications
        SET read = true
        WHERE id = $1 AND recipient_id = $2
        "#,
    )
    .bind(notification_id)
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn mark_all_notifications_read(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE notifications
        SET read = true
        WHERE recipient_id = $1 AND read = false
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}

pub async fn get_unread_count(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<i64, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM notifications
        WHERE recipient_id = $1 AND read = false
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;

    Ok(count)
}

pub async fn get_workout_owner(
    pool: &PgPool,
    workout_id: Uuid,
) -> Result<Option<Uuid>, sqlx::Error> {
    let owner_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT user_id
        FROM workout_data
        WHERE id = $1
        "#,
    )
    .bind(workout_id)
    .fetch_optional(pool)
    .await?;

    Ok(owner_id)
}