use actix_web::{web, HttpResponse, Result};
use sqlx::{PgPool, Row};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

use crate::handlers::admin::user_handler::ApiResponse;

#[derive(Serialize)]
pub struct AdminLeagueResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub max_teams: i32,
    pub current_team_count: i64,
    pub season_start_date: DateTime<Utc>,
    pub season_end_date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct CreateLeagueRequest {
    pub name: String,
    pub description: Option<String>,
    pub max_teams: i32,
    pub season_start_date: DateTime<Utc>,
    pub season_end_date: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct UpdateLeagueRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub max_teams: Option<i32>,
    pub season_start_date: Option<DateTime<Utc>>,
    pub season_end_date: Option<DateTime<Utc>>,
    pub is_active: Option<bool>,
}

// GET /admin/leagues - Get all leagues
pub async fn get_leagues(
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    // For now, we'll use the league_seasons table as our leagues
    // In a production system, you might have a separate leagues table
    let rows = sqlx::query(r#"
        SELECT 
            ls.id,
            ls.name,
            ls.start_date as season_start_date,
            ls.end_date as season_end_date,
            ls.is_active,
            ls.created_at,
            COUNT(DISTINCT t.id) as current_team_count
        FROM league_seasons ls
        LEFT JOIN teams t ON 1=1  -- TODO: Add proper league-team relationship
        GROUP BY ls.id, ls.name, ls.start_date, ls.end_date, ls.is_active, ls.created_at
        ORDER BY ls.created_at DESC
    "#)
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        eprintln!("Database error getting leagues: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    let leagues: Vec<AdminLeagueResponse> = rows
        .into_iter()
        .map(|row| AdminLeagueResponse {
            id: row.get("id"),
            name: row.get("name"),
            description: None, // TODO: Add description field to league_seasons table
            is_active: row.get("is_active"),
            max_teams: 12, // Default max teams for now
            current_team_count: row.get::<i64, _>("current_team_count"),
            season_start_date: row.get("season_start_date"),
            season_end_date: row.get("season_end_date"),
            created_at: row.get("created_at"),
        })
        .collect();

    let response = ApiResponse {
        data: leagues,
        success: true,
        message: None,
    };

    Ok(HttpResponse::Ok().json(response))
}

// GET /admin/leagues/{id} - Get league by ID
pub async fn get_league_by_id(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let league_id = path.into_inner();

    let row = sqlx::query(r#"
        SELECT 
            ls.id,
            ls.name,
            ls.start_date as season_start_date,
            ls.end_date as season_end_date,
            ls.is_active,
            ls.created_at,
            COUNT(DISTINCT t.id) as current_team_count
        FROM league_seasons ls
        LEFT JOIN teams t ON 1=1  -- TODO: Add proper league-team relationship
        WHERE ls.id = $1
        GROUP BY ls.id, ls.name, ls.start_date, ls.end_date, ls.is_active, ls.created_at
    "#)
    .bind(league_id)
    .fetch_optional(pool.get_ref())
    .await
    .map_err(|e| {
        eprintln!("Database error getting league: {}", e);
        actix_web::error::ErrorInternalServerError("Database error")
    })?;

    if let Some(row) = row {
        let league = AdminLeagueResponse {
            id: row.get("id"),
            name: row.get("name"),
            description: None,
            is_active: row.get("is_active"),
            max_teams: 12,
            current_team_count: row.get::<i64, _>("current_team_count"),
            season_start_date: row.get("season_start_date"),
            season_end_date: row.get("season_end_date"),
            created_at: row.get("created_at"),
        };

        let response = ApiResponse {
            data: league,
            success: true,
            message: None,
        };

        Ok(HttpResponse::Ok().json(response))
    } else {
        Ok(HttpResponse::NotFound().json(serde_json::json!({
            "error": "League not found"
        })))
    }
}

// POST /admin/leagues - Create new league
pub async fn create_league(
    pool: web::Data<PgPool>,
    body: web::Json<CreateLeagueRequest>,
) -> Result<HttpResponse> {
    let league_id = Uuid::new_v4();
    let now = chrono::Utc::now();

    // Validate dates
    if body.season_start_date >= body.season_end_date {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Season start date must be before end date"
        })));
    }

    let result = sqlx::query!(
        r#"
        INSERT INTO league_seasons (id, name, start_date, end_date, is_active, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
        league_id,
        body.name,
        body.season_start_date,
        body.season_end_date,
        true, // New leagues are active by default
        now,
        now
    )
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            let league = AdminLeagueResponse {
                id: league_id,
                name: body.name.clone(),
                description: body.description.clone(),
                is_active: true,
                max_teams: body.max_teams,
                current_team_count: 0,
                season_start_date: body.season_start_date,
                season_end_date: body.season_end_date,
                created_at: now,
            };

            let response = ApiResponse {
                data: league,
                success: true,
                message: Some("League created successfully".to_string()),
            };

            Ok(HttpResponse::Created().json(response))
        }
        Err(e) => {
            eprintln!("Database error creating league: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to create league"
            })))
        }
    }
}

// PATCH /admin/leagues/{id} - Update league
pub async fn update_league(
    pool: web::Data<PgPool>,
    path: web::Path<Uuid>,
    body: web::Json<UpdateLeagueRequest>,
) -> Result<HttpResponse> {
    let league_id = path.into_inner();

    let mut updates = Vec::new();
    let mut params: Vec<Box<dyn std::any::Any + Send>> = Vec::new();
    let mut param_count = 1;

    if let Some(name) = &body.name {
        updates.push(format!("name = ${}", param_count));
        params.push(Box::new(name.clone()));
        param_count += 1;
    }

    if let Some(start_date) = &body.season_start_date {
        updates.push(format!("start_date = ${}", param_count));
        params.push(Box::new(*start_date));
        param_count += 1;
    }

    if let Some(end_date) = &body.season_end_date {
        updates.push(format!("end_date = ${}", param_count));
        params.push(Box::new(*end_date));
        param_count += 1;
    }

    if let Some(is_active) = &body.is_active {
        updates.push(format!("is_active = ${}", param_count));
        params.push(Box::new(*is_active));
        param_count += 1;
    }

    if updates.is_empty() {
        return Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "No fields to update"
        })));
    }

    updates.push(format!("updated_at = ${}", param_count));
    let now = chrono::Utc::now();
    params.push(Box::new(now));
    param_count += 1;

    // For simplicity, let's use a direct query approach
    let mut query_builder = sqlx::QueryBuilder::new("UPDATE league_seasons SET ");
    let mut separator = "";

    if let Some(name) = &body.name {
        query_builder.push(separator);
        query_builder.push("name = ");
        query_builder.push_bind(name);
        separator = ", ";
    }

    if let Some(start_date) = &body.season_start_date {
        query_builder.push(separator);
        query_builder.push("start_date = ");
        query_builder.push_bind(start_date);
        separator = ", ";
    }

    if let Some(end_date) = &body.season_end_date {
        query_builder.push(separator);
        query_builder.push("end_date = ");
        query_builder.push_bind(end_date);
        separator = ", ";
    }

    if let Some(is_active) = &body.is_active {
        query_builder.push(separator);
        query_builder.push("is_active = ");
        query_builder.push_bind(is_active);
        separator = ", ";
    }

    query_builder.push(separator);
    query_builder.push("updated_at = ");
    query_builder.push_bind(now);

    query_builder.push(" WHERE id = ");
    query_builder.push_bind(league_id);

    let result = query_builder.build().execute(pool.get_ref()).await;

    match result {
        Ok(result) => {
            if result.rows_affected() > 0 {
                // Fetch updated league
                let league = get_league_by_id(pool, web::Path::from(league_id)).await?;
                Ok(league)
            } else {
                Ok(HttpResponse::NotFound().json(serde_json::json!({
                    "error": "League not found"
                })))
            }
        }
        Err(e) => {
            eprintln!("Database error updating league: {}", e);
            Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Failed to update league"
            })))
        }
    }
}