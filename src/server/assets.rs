use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "src/assets/"]
pub struct Assets;

pub async fn serve_dashboard() -> Response {
    match Assets::get("dashboard.html") {
        Some(content) => {
            let body = content.data.into_owned();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                body,
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "Dashboard not found").into_response(),
    }
}

#[allow(dead_code)]
pub async fn serve_asset(path: &str) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();

    match Assets::get(path) {
        Some(content) => {
            let body = content.data.into_owned();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime.as_ref())],
                body,
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "Asset not found").into_response(),
    }
}
