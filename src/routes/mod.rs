use actix_web::web;

pub mod registration;
pub mod backend_health;
pub mod auth;
pub mod health_data;
pub mod websocket;
pub mod league;
pub mod profile;
pub mod workout_sync;
pub mod admin;
pub mod social;
pub mod feed;
pub mod posts;
pub mod media;
pub mod analytics;
pub mod notifications;

use crate::middleware::auth::AuthMiddleware;

pub fn init_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(registration::register)
        .service(backend_health::backend_health)
        .service(auth::login)
        .service(auth::biometric_refresh)
        .service(auth::reset_password_route);
    // Health routes (require authentication)
    cfg.service(
        web::scope("/health")
            .wrap(AuthMiddleware)
            .service(health_data::upload_health)
            .service(workout_sync::get_workout_hist)
            .service(workout_sync::get_workout_detail_handler)
            .service(workout_sync::check_workout_sync_handler)
            .service(workout_sync::submit_scoring_feedback_handler)
            .service(workout_sync::get_scoring_feedback_handler)
            .service(workout_sync::submit_workout_report_handler)
            .service(workout_sync::get_my_report_for_workout_handler)
            .service(workout_sync::get_my_reports_handler)
            .service(workout_sync::delete_workout_report_handler)
    );
    // Profile routes (require authentication)
    cfg.service(
        web::scope("/profile")
            .wrap(AuthMiddleware)
            .service(profile::get_user)
            .service(profile::get_health_prof)
            .service(profile::update_health_prof)
            .service(profile::request_profile_picture_upload_url_handler)
            .service(profile::confirm_profile_picture_upload_handler)
            .service(profile::get_profile_picture_download_url_handler)
            .service(profile::serve_profile_picture)
            .service(profile::get_status)
            .service(profile::update_status)
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
            .service(league::get_live_active_games)
            .service(league::get_recent_results)
            .service(league::get_game_week)
            .service(league::register_team)
            .service(league::get_user_team)
            .service(league::get_team_info)
            .service(league::get_all_teams)
            .service(league::update_team)
            .service(league::get_team_history)
            .service(league::add_team_member)
            .service(league::get_team_members)
            .service(league::remove_team_member)
            .service(league::update_my_team_status)
            .service(league::update_team_member)
            .service(league::get_league_users_with_stats)
            .service(league::get_live_scores)
            .service(league::get_game_live_score)
            .service(league::get_game_player_scores)
            .service(league::get_active_games)
            .service(league::manage_games)
            .service(league::get_game_summary)
            .service(league::get_player_pool)
            .service(league::send_team_invitation)
            .service(league::get_user_invitations)
            .service(league::respond_to_invitation)
            .service(league::create_team_poll)
            .service(league::get_team_polls)
            .service(league::cast_poll_vote)
            .service(league::delete_poll)
            .service(league::send_team_chat)
            .service(league::get_team_chat)
            .service(league::get_unread_chat_count)
            .service(league::mark_team_chat_read)  // Must come before edit/delete to avoid UUID parsing conflict
            .service(league::edit_team_chat)
            .service(league::delete_team_chat)
    );
    // WebSocket routes (authentication handled in route)
    cfg.service(
        web::resource("/game-ws")
            .route(web::get().to(websocket::game_ws_route))
    );
    
    // Admin routes (require admin authentication)
    admin::init_admin_routes(cfg);

    // Social routes (require authentication)
    cfg.service(
        web::scope("/social")
            .wrap(AuthMiddleware)
            .configure(social::init_social_routes)
    );

    // Feed routes (require authentication)
    cfg.service(
        web::scope("/feed")
            .wrap(AuthMiddleware)
            .configure(feed::init_feed_routes)
    );

    // Posts routes (require authentication)
    cfg.service(
        web::scope("/posts")
            .wrap(AuthMiddleware)
            .configure(posts::init_posts_routes)
    );

    // Media routes (require authentication)
    cfg.service(
        web::scope("/media")
            .wrap(AuthMiddleware)
            .configure(media::init_media_routes)
    );

    // Analytics routes (require authentication)
    cfg.service(
        web::scope("/analytics")
            .wrap(AuthMiddleware)
            .configure(analytics::init_analytics_routes)
    );

    // Notification routes (require authentication)
    cfg.service(
        web::scope("/notifications")
            .wrap(AuthMiddleware)
            .configure(notifications::init_notification_routes)
    );
}