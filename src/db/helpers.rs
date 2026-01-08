//! Database query helper functions to reduce boilerplate error handling.
//!
//! These helpers simplify common patterns like:
//! - Fetching a required record (NotFound if missing)
//! - Ensuring a record doesn't exist (Conflict if it does)
//!
//! # Usage
//!
//! For handlers returning `HttpResponse`:
//! ```ignore
//! let user = require_record(query.fetch_optional(pool).await, "User not found")?;
//! ```
//!
//! For handlers returning `Result<HttpResponse>`:
//! ```ignore
//! let user = ok_or_return!(require_record(query.fetch_optional(pool).await, "User not found"));
//! ```

use actix_web::HttpResponse;
use serde::Serialize;
use serde_json::json;

/// Macro for handlers returning `Result<HttpResponse>`.
/// Converts a `DbResult<T>` to return `Ok(error_response)` on error.
///
/// # Example
/// ```ignore
/// let user = ok_or_return!(require_record(
///     sqlx::query!(...).fetch_optional(pool).await,
///     "User not found"
/// ));
/// ```
#[macro_export]
macro_rules! ok_or_return {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(response) => return Ok(response),
        }
    };
}

/// Result type for database operations that return an HttpResponse on error
pub type DbResult<T> = Result<T, HttpResponse>;

/// Unwrap an optional database result, returning NotFound if None.
///
/// # Example
/// ```ignore
/// let user = require_record(
///     sqlx::query!("SELECT * FROM users WHERE id = $1", user_id)
///         .fetch_optional(pool)
///         .await,
///     "User not found"
/// )?;
/// ```
pub fn require_record<T>(
    result: Result<Option<T>, sqlx::Error>,
    not_found_message: &str,
) -> DbResult<T> {
    match result {
        Ok(Some(record)) => Ok(record),
        Ok(None) => Err(HttpResponse::NotFound().json(json!({
            "success": false,
            "message": not_found_message
        }))),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            Err(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Database error"
            })))
        }
    }
}

/// Ensure a record does NOT exist, returning Conflict if it does.
///
/// # Example
/// ```ignore
/// ensure_not_exists(
///     sqlx::query!("SELECT id FROM teams WHERE name = $1", name)
///         .fetch_optional(pool)
///         .await,
///     "Team name already taken"
/// )?;
/// ```
pub fn ensure_not_exists<T>(
    result: Result<Option<T>, sqlx::Error>,
    conflict_message: &str,
) -> DbResult<()> {
    match result {
        Ok(Some(_)) => Err(HttpResponse::Conflict().json(json!({
            "success": false,
            "message": conflict_message
        }))),
        Ok(None) => Ok(()),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            Err(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Database error"
            })))
        }
    }
}

/// Unwrap a database result, returning InternalServerError on error.
/// Use this when you just need to handle the Err case.
///
/// # Example
/// ```ignore
/// let users = db_result(
///     sqlx::query!("SELECT * FROM users")
///         .fetch_all(pool)
///         .await
/// )?;
/// ```
pub fn db_result<T>(result: Result<T, sqlx::Error>) -> DbResult<T> {
    result.map_err(|e| {
        tracing::error!("Database error: {}", e);
        HttpResponse::InternalServerError().json(json!({
            "success": false,
            "message": "Database error"
        }))
    })
}

/// Similar to require_record but with custom response structure using ApiResponse.
pub fn require_record_api<T, R: Serialize>(
    result: Result<Option<T>, sqlx::Error>,
    not_found_response: R,
) -> DbResult<T> {
    match result {
        Ok(Some(record)) => Ok(record),
        Ok(None) => Err(HttpResponse::NotFound().json(not_found_response)),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            Err(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Database error"
            })))
        }
    }
}

/// Similar to ensure_not_exists but with custom response structure.
pub fn ensure_not_exists_api<T, R: Serialize>(
    result: Result<Option<T>, sqlx::Error>,
    conflict_response: R,
) -> DbResult<()> {
    match result {
        Ok(Some(_)) => Err(HttpResponse::Conflict().json(conflict_response)),
        Ok(None) => Ok(()),
        Err(e) => {
            tracing::error!("Database error: {}", e);
            Err(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "Database error"
            })))
        }
    }
}
