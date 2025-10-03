use actix_web::web;

use crate::handlers::social::{
    reaction_handler::{add_reaction, remove_reaction, get_reactions, get_reaction_details},
    comment_handler::{add_comment, edit_comment, remove_comment, get_comments, get_single_comment},
    comment_reaction_handler::{add_comment_reaction, remove_comment_reaction, get_comment_reactions_handler, get_comment_reaction_details},
    notification_handler::{get_user_notifications, mark_notification_as_read, mark_all_as_read, get_unread_notification_count},
};

pub fn init_social_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/workouts/{workout_id}")
            // Reaction endpoints
            .service(
                web::resource("/reactions")
                    .route(web::post().to(add_reaction))
                    .route(web::delete().to(remove_reaction))
                    .route(web::get().to(get_reactions))
            )
            .service(
                web::resource("/reactions/users")
                    .route(web::get().to(get_reaction_details))
            )
            // Comment endpoints
            .service(
                web::resource("/comments")
                    .route(web::post().to(add_comment))
                    .route(web::get().to(get_comments))
            )
    );

    // Comment-specific endpoints (not nested under workout)
    cfg.service(
        web::resource("/comments/{comment_id}")
            .route(web::put().to(edit_comment))
            .route(web::delete().to(remove_comment))
            .route(web::get().to(get_single_comment))
    );

    // Comment reaction endpoints
    cfg.service(
        web::resource("/comments/{comment_id}/reactions")
            .route(web::post().to(add_comment_reaction))
            .route(web::delete().to(remove_comment_reaction))
            .route(web::get().to(get_comment_reactions_handler))
    );

    cfg.service(
        web::resource("/comments/{comment_id}/reactions/users")
            .route(web::get().to(get_comment_reaction_details))
    );

    // Notification endpoints
    cfg.service(
        web::resource("/notifications")
            .route(web::get().to(get_user_notifications))
    );

    cfg.service(
        web::resource("/notifications/unread-count")
            .route(web::get().to(get_unread_notification_count))
    );

    cfg.service(
        web::resource("/notifications/{notification_id}/read")
            .route(web::put().to(mark_notification_as_read))
    );

    cfg.service(
        web::resource("/notifications/mark-all-read")
            .route(web::put().to(mark_all_as_read))
    );
}