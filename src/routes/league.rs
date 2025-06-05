// src/routes/league.rs
use actix_web::{get, post, put, web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;

use crate::handlers::league::{team_handler, game_handler, league_handler, season_handler};
use crate::middleware::auth::Claims;
use crate::models::league::*;
use crate::models::team::{TeamRegistrationRequest, TeamUpdateRequest};

/// Create a new league season
#[post("/season_create")]
async fn create_season(
    season_request: web::Json<CreateSeasonRequest>,
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    league_handler::create_league_season(season_request, pool, claims).await
}

/// Get active season information
#[get("/seasons/active")]
async fn get_active_season(
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    season_handler::get_active_league_season(pool).await
}

/// Get specific season by ID
#[get("/seasons/{season_id}")]
async fn get_season(
    path: web::Path<Uuid>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let season_id = path.into_inner();
    season_handler::get_league_season(season_id, pool).await
}

/// Get all seasons (with pagination)
#[get("/seasons")]
async fn get_all_seasons(
    query: web::Query<PaginationQuery>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    season_handler::get_all_league_seasons(query, pool).await
}

/// Get season schedule
#[get("/seasons/{season_id}/schedule")]
async fn get_season_schedule(
    path: web::Path<Uuid>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let season_id = path.into_inner();
    season_handler::get_league_schedule(season_id, pool).await
}

/// Get season standings
#[get("/seasons/{season_id}/standings")]
async fn get_season_standings(
    path: web::Path<Uuid>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let season_id = path.into_inner();
    season_handler::get_league_standings(season_id, pool).await
}

/// Update game result
#[put("/games/{game_id}/result")]
async fn update_game_result(
    path: web::Path<Uuid>,
    result_request: web::Json<GameResultRequest>,
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let game_id = path.into_inner();
    game_handler::update_league_game_result(game_id, result_request, pool, claims).await
}

/// Get next game countdown
#[get("/game_countdown")]
async fn get_countdown_info(
    query: web::Query<CountdownQuery>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    game_handler::get_game_countdown(query, pool).await
}

/// Get upcoming games
#[get("/games/upcoming")]
async fn get_upcoming_games(
    query: web::Query<UpcomingGamesQuery>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    game_handler::get_league_upcoming_games(query, pool).await
}

/// Get recent results
#[get("/games/results")]
async fn get_recent_results(
    query: web::Query<RecentResultsQuery>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    league_handler::get_league_recent_results(query, pool).await
}

/// Get games for specific week
#[get("/seasons/{season_id}/weeks/{week_number}")]
async fn get_game_week(
    path: web::Path<(Uuid, i32)>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let (season_id, week_number) = path.into_inner();
    league_handler::get_league_game_week(season_id, week_number, pool).await
}

/// Register a new team for league participation
#[post("/teams/register")]
async fn register_team(
    team_request: web::Json<TeamRegistrationRequest>,
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    team_handler::register_new_team(team_request, pool, claims).await
}

/// Get team information
#[get("/teams/{team_id}")]
async fn get_team_info(
    path: web::Path<Uuid>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let team_id = path.into_inner();
    team_handler::get_team_information(team_id, pool).await
}

/// Get all registered teams
#[get("/teams")]
async fn get_all_teams(
    query: web::Query<PaginationQuery>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    team_handler::get_all_registered_teams(query, pool).await
}

/// Update team information
#[put("/teams/{team_id}")]
async fn update_team(
    path: web::Path<Uuid>,
    team_update: web::Json<TeamUpdateRequest>,
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    let team_id = path.into_inner();
    team_handler::update_team_information(team_id, team_update, pool, claims).await
}

/// Get team's league history
#[get("/teams/{team_id}/history")]
async fn get_team_history(
    path: web::Path<Uuid>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    let team_id = path.into_inner();
    team_handler::get_team_league_history(team_id, pool).await
}