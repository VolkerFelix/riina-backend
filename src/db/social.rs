use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::models::social::{
    CommentListResponse, WorkoutComment, WorkoutCommentWithUser, WorkoutReaction,
    WorkoutReactionWithUser, ReactionSummary,
};

pub async fn create_reaction(
    pool: &PgPool,
    user_id: Uuid,
    workout_id: Uuid,
    reaction_type: &str,
) -> Result<WorkoutReaction, sqlx::Error> {
    let reaction = sqlx::query_as::<_, WorkoutReaction>(
        r#"
        INSERT INTO workout_reactions (user_id, workout_id, reaction_type)
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
        DELETE FROM workout_reactions
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
) -> Result<Vec<ReactionSummary>, sqlx::Error> {
    let summaries = sqlx::query(
        r#"
        SELECT
            wr.reaction_type,
            COUNT(wr.id) as count,
            BOOL_OR(wr.user_id = $2) as user_reacted
        FROM workout_reactions wr
        WHERE wr.workout_id = $1
        GROUP BY wr.reaction_type
        "#,
    )
    .bind(workout_id)
    .bind(current_user_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|row| ReactionSummary {
        reaction_type: row.get("reaction_type"),
        count: row.get("count"),
        user_reacted: row.get("user_reacted"),
    })
    .collect();

    Ok(summaries)
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
            FROM workout_reactions wr
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
            FROM workout_reactions wr
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
        INSERT INTO workout_comments (user_id, workout_id, content, parent_id)
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
        UPDATE workout_comments
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
        DELETE FROM workout_comments
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
) -> Result<CommentListResponse, sqlx::Error> {
    let offset = (page - 1) * per_page;

    let total_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM workout_comments
        WHERE workout_id = $1 AND parent_id IS NULL
        "#,
    )
    .bind(workout_id)
    .fetch_one(pool)
    .await?;

    // Get top-level comments first
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
            c.updated_at
        FROM workout_comments c
        INNER JOIN users u ON u.id = c.user_id
        WHERE c.workout_id = $1 AND c.parent_id IS NULL
        ORDER BY c.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(workout_id)
    .bind(per_page)
    .bind(offset)
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
        };

        // Get replies for this comment
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
                c.updated_at
            FROM workout_comments c
            INNER JOIN users u ON u.id = c.user_id
            WHERE c.parent_id = $1
            ORDER BY c.created_at ASC
            "#,
        )
        .bind(comment.id)
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
        FROM workout_comments c
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
    });

    Ok(comment)
}