use actix_web::web;

use crate::handlers::media::media_upload;

pub fn init_media_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/upload-url")
            .route(web::post().to(media_upload::request_upload_signed_url))
    );
    cfg.service(
        web::resource("/confirm-upload")
            .route(web::post().to(media_upload::confirm_upload))
    );
    cfg.service(
        web::resource("/download-url/{user_id}/{filename}")
            .route(web::get().to(media_upload::get_download_signed_url))
    );
}
