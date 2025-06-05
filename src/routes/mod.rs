use actix_web::web;

pub mod registration;
pub mod backend_health;
pub mod auth;
pub mod protected;
pub mod health_data;
pub mod websocket;
pub mod league;

use crate::middleware::auth::AuthMiddleware;

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(registration::register)
        .service(backend_health::backend_health)
        .service(auth::login);

    cfg.service(
        web::scope("/protected")
            .service(protected::protected_resource)
    );
    // Health routes (require authentication)
    cfg.service(
        web::scope("/health")
            .wrap(AuthMiddleware)
            .service(health_data::upload_health)
    );
    // League routes (require authentication)
    cfg.service(
        web::scope("/league")
            .wrap(AuthMiddleware)
            .service(league::create_season)
            .service(league::get_active_season)
            .service(league::get_season)
            .service(league::get_all_seasons)
            .service(league::get_season_schedule)
            .service(league::get_season_standings)
            .service(league::update_game_result)
            .service(league::get_countdown_info)
            .service(league::get_upcoming_games)
            .service(league::get_recent_results)
            .service(league::get_game_week)
            .service(league::register_team)
            .service(league::get_team_info)
            .service(league::get_all_teams)
            .service(league::update_team)
            .service(league::get_team_history)
            .service(league::add_team_member)
            .service(league::get_team_members)
            .service(league::remove_team_member)
            .service(league::update_team_member)
    );
    // WebSocket routes (authentication handled in route)
    cfg.service(
        web::resource("/game-ws")
            .route(web::get().to(websocket::game_ws_route))
    );
}