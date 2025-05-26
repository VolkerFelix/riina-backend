use actix_web::{web, Scope};

use crate::handlers::llm_handler::{
    generate_twin_thought,
    handle_user_response,
    get_twin_history,
    update_user_reaction,
    trigger_health_reaction,
};

pub fn llm_routes() -> Scope {
    web::scope("/llm")
        .route("/generate_thought", web::post().to(generate_twin_thought))
        .route("/user_response", web::post().to(handle_user_response))
        .route("/history", web::get().to(get_twin_history))
        .route("/reaction", web::post().to(update_user_reaction))
        .route("/health_reaction", web::post().to(trigger_health_reaction))

}