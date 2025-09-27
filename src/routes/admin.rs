use actix_web::web;

use crate::handlers::admin::{
    user_handler,
    team_handler,
    league_handler,
    game_management_handler,
    workout_handler,
};
use crate::middleware::admin::AdminMiddleware;

pub fn init_admin_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/admin")
            .wrap(AdminMiddleware)
            // User management routes
            .service(
                web::resource("/users")
                    .route(web::get().to(user_handler::get_users))
            )
            .service(
                web::resource("/users/without-team")
                    .route(web::get().to(user_handler::get_users_without_team))
            )
            .service(
                web::resource("/users/{id}")
                    .route(web::get().to(user_handler::get_user_by_id))
                    .route(web::delete().to(user_handler::delete_user))
            )
            .service(
                web::resource("/users/{id}/status")
                    .route(web::patch().to(user_handler::update_user_status))
            )
            
            // Team management routes
            .service(
                web::resource("/teams")
                    .route(web::get().to(team_handler::get_teams))
                    .route(web::post().to(team_handler::create_team))
            )
            .service(
                web::resource("/teams/{id}")
                    .route(web::get().to(team_handler::get_team_by_id))
                    .route(web::patch().to(team_handler::update_team))
                    .route(web::delete().to(team_handler::delete_team))
            )
            .service(
                web::resource("/teams/{id}/members")
                    .route(web::get().to(team_handler::get_team_members))
                    .route(web::post().to(team_handler::add_team_member))
            )
            .service(
                web::resource("/teams/{team_id}/members/{member_id}")
                    .route(web::patch().to(team_handler::update_team_member))
                    .route(web::delete().to(team_handler::remove_team_member))
            )
            
            // League management routes
            .service(
                web::resource("/leagues")
                    .route(web::get().to(league_handler::get_leagues))
                    .route(web::post().to(league_handler::create_league))
            )
            .service(
                web::resource("/leagues/{id}")
                    .route(web::get().to(league_handler::get_league_by_id))
                    .route(web::patch().to(league_handler::update_league))
            )
            .service(
                web::resource("/leagues/{id}/teams")
                    .route(web::get().to(league_handler::get_league_teams))
                    .route(web::post().to(league_handler::assign_team_to_league))
                    .route(web::delete().to(league_handler::remove_team_from_league))
            )
            // Season management routes
            .service(
                web::resource("/leagues/{id}/seasons")
                    .route(web::get().to(league_handler::get_league_seasons))
                    .route(web::post().to(league_handler::create_league_season))
            )
            .service(
                web::resource("/leagues/{id}/seasons/{season_id}")
                    .route(web::get().to(league_handler::get_league_season_by_id))
                    .route(web::patch().to(league_handler::update_league_season))
                    .route(web::delete().to(league_handler::delete_league_season))
            )
            // Game management routes
            .service(
                web::resource("/games/start-now")
                    .route(web::post().to(game_management_handler::start_games_now))
            )
            .service(
                web::resource("/games/status/{season_id}")
                    .route(web::get().to(game_management_handler::get_games_status))
            )
            .service(
                web::resource("/games/adjust-score")
                    .route(web::post().to(game_management_handler::adjust_live_game_score))
            )
            .service(
                web::resource("/games/evaluate")
                    .route(web::post().to(game_management_handler::evaluate_games_for_date))
            )
            .service(
                web::resource("/games/trigger-evaluation")
                    .route(web::post().to(game_management_handler::trigger_game_evaluation))
            )
            .service(
                web::resource("/games/finish-ongoing")
                    .route(web::post().to(game_management_handler::finish_ongoing_games))
            )
            
            // Workout management routes
            .service(
                web::resource("/workouts")
                    .route(web::get().to(workout_handler::get_all_workouts))
            )
            .service(
                web::resource("/workouts/bulk-delete")
                    .route(web::post().to(workout_handler::bulk_delete_workouts))
            )
            .service(
                web::resource("/workouts/{id}")
                    .route(web::get().to(workout_handler::get_workout_detail))
                    .route(web::delete().to(workout_handler::delete_workout))
            )
    );
}