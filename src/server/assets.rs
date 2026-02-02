use crate::server::auth::JwtManager;
use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "src/assets/"]
pub struct Assets;

/// Dashboard session token TTL in hours
const DASHBOARD_TOKEN_TTL_HOURS: i64 = 1;

pub async fn serve_dashboard_with_token(jwt_secret: &str) -> Response {
    match Assets::get("dashboard.html") {
        Some(content) => {
            let html = String::from_utf8_lossy(&content.data);

            // Generate session token for dashboard API access
            let jwt_manager = JwtManager::new(jwt_secret);
            let token = jwt_manager
                .generate_token("dashboard", DASHBOARD_TOKEN_TTL_HOURS)
                .unwrap_or_default();

            // Inject token into HTML before </head>
            let token_script = format!(
                r#"<script>window.INFRACTL_TOKEN="{}";</script></head>"#,
                token
            );
            let html = html.replace("</head>", &token_script);

            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                html.into_bytes(),
            )
                .into_response()
        }
        None => (StatusCode::NOT_FOUND, "Dashboard not found").into_response(),
    }
}

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
