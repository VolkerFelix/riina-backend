// src/routes/league.rs
use actix_web::{delete, get, post, put, web, HttpResponse, Result};
use sqlx::PgPool;
use uuid::Uuid;
use std::sync::Arc;
use redis::Client as RedisClient;

use crate::handlers::league::{team_handler, team_member_handler, game_handler, league_handler, season_handler, league_users_handler};
use crate::handlers::league::league_users_handler::PaginationParams;
use crate::middleware::auth::Claims;
use crate::models::league::*;
use crate::models::team::{TeamRegistrationRequest, TeamUpdateRequest, AddTeamMemberRequest, UpdateTeamMemberRequest, CreatePollRequest, CastVoteRequest};

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

/// Get live/active games (similar format to upcoming games)
#[get("/games/live-active")]
async fn get_live_active_games(
    query: web::Query<UpcomingGamesQuery>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    game_handler::get_league_live_games(query, pool).await
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

/// Get current user's team
#[get("/teams/my-team")]
async fn get_user_team(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    team_handler::get_user_team(pool, claims).await
}

/// Register a new team for league participation
#[post("/teams/register")]
async fn register_team(
    team_request: web::Json<TeamRegistrationRequest>,
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<RedisClient>>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    team_handler::register_new_team(team_request, pool, redis_client, claims).await
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

/// Add a user to a team
#[post("/teams/{team_id}/members")]
async fn add_team_member(
    path: web::Path<Uuid>,
    request: web::Json<AddTeamMemberRequest>,
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<RedisClient>>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    team_member_handler::add_team_member(path, request, pool, redis_client, claims).await
}

/// Get all members of a team
#[get("/teams/{team_id}/members")]
async fn get_team_members(
    path: web::Path<Uuid>,
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    team_member_handler::get_team_members(path, pool, claims).await
}

/// Remove a user from a team
#[actix_web::delete("/teams/{team_id}/members/{user_id}")]
async fn remove_team_member(
    path: web::Path<(Uuid, Uuid)>,
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<RedisClient>>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    team_member_handler::remove_team_member(path, pool, redis_client, claims).await
}

/// Update a team member's role or status
#[put("/teams/{team_id}/members/{user_id}")]
async fn update_team_member(
    path: web::Path<(Uuid, Uuid)>,
    request: web::Json<UpdateTeamMemberRequest>,
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    team_member_handler::update_team_member(path, request, pool, claims).await
}

/// Get all league users with their stats
#[get("/users/stats")]
async fn get_league_users_with_stats(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
    query: web::Query<PaginationParams>
) -> Result<HttpResponse> {
    league_users_handler::get_league_users_with_stats(pool, claims, query).await
}

/// Get live scores for all active games
#[get("/games/live")]
async fn get_live_scores(
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<redis::Client>>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    use crate::handlers::league::live_game_handler;
    live_game_handler::get_live_scores(pool, redis_client, claims).await
}

/// Get live score for a specific game
#[get("/games/{game_id}/live")]
async fn get_game_live_score(
    path: web::Path<Uuid>,
    query: web::Query<crate::models::league::PaginationQuery>,
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    use crate::handlers::league::live_game_handler;
    live_game_handler::get_game_live_score(pool, path, query, claims).await
}

/// Get all currently active games
#[get("/games/active")]
async fn get_active_games(
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<redis::Client>>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    use crate::handlers::league::live_game_handler;
    live_game_handler::get_active_games(pool, redis_client, claims).await
}

/// Admin endpoint to manage games (start/finish)
#[post("/games/manage")]
async fn manage_games(
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<redis::Client>>,
) -> Result<HttpResponse> {
    use crate::handlers::league::live_game_handler;
    live_game_handler::manage_games(pool, redis_client).await
}

/// Get game summary
#[get("/games/{game_id}/summary")]
async fn get_game_summary(
    path: web::Path<Uuid>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse> {
    game_handler::get_game_summary(path, pool).await
}

/// Get player pool (active users not in teams)
#[get("/player-pool")]
async fn get_player_pool(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    use crate::handlers::league::player_pool_handler;
    Ok(player_pool_handler::get_player_pool(pool, claims).await)
}

/// Send team invitation to a free agent
#[post("/teams/{team_id}/invitations")]
async fn send_team_invitation(
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<RedisClient>>,
    claims: web::ReqData<Claims>,
    team_id: web::Path<Uuid>,
    request: web::Json<crate::models::team_invitation::SendInvitationRequest>,
) -> Result<HttpResponse> {
    use crate::handlers::league::team_invitation_handler;
    Ok(team_invitation_handler::send_invitation(pool, redis_client, claims, team_id, request).await)
}

/// Get user's team invitations
#[get("/invitations")]
async fn get_user_invitations(
    pool: web::Data<PgPool>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    use crate::handlers::league::team_invitation_handler;
    Ok(team_invitation_handler::get_user_invitations(pool, claims).await)
}

/// Respond to team invitation
#[post("/invitations/{invitation_id}/respond")]
async fn respond_to_invitation(
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<RedisClient>>,
    claims: web::ReqData<Claims>,
    invitation_id: web::Path<Uuid>,
    request: web::Json<crate::models::team_invitation::RespondToInvitationRequest>,
) -> Result<HttpResponse> {
    use crate::handlers::league::team_invitation_handler;
    Ok(team_invitation_handler::respond_to_invitation(pool, redis_client, claims, invitation_id, request).await)
}

/// Create a poll to remove a team member
#[post("/teams/{team_id}/polls")]
async fn create_team_poll(
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<RedisClient>>,
    team_id: web::Path<Uuid>,
    request: web::Json<CreatePollRequest>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    use crate::handlers::league::team_poll_handler;
    Ok(team_poll_handler::create_poll(request, pool, redis_client, team_id, claims).await)
}

/// Get active polls for a team
#[get("/teams/{team_id}/polls")]
async fn get_team_polls(
    pool: web::Data<PgPool>,
    team_id: web::Path<Uuid>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    use crate::handlers::league::team_poll_handler;
    Ok(team_poll_handler::get_team_polls(pool, team_id, claims).await)
}

/// Cast a vote on a poll
#[post("/teams/{team_id}/polls/{poll_id}/vote")]
async fn cast_poll_vote(
    pool: web::Data<PgPool>,
    redis_client: web::Data<Arc<RedisClient>>,
    path: web::Path<(Uuid, Uuid)>,
    request: web::Json<CastVoteRequest>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    use crate::handlers::league::team_poll_handler;
    Ok(team_poll_handler::cast_vote(request, pool, redis_client, path, claims).await)
}

/// Delete a poll (creator only)
#[delete("/teams/{team_id}/polls/{poll_id}")]
async fn delete_poll(
    pool: web::Data<PgPool>,
    path: web::Path<(Uuid, Uuid)>,
    claims: web::ReqData<Claims>,
) -> Result<HttpResponse> {
    use crate::handlers::league::team_poll_handler;
    Ok(team_poll_handler::delete_poll(pool, path, claims).await)
}

