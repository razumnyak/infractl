use crate::config::is_ip_allowed;
use crate::server::auth::JwtManager;
use crate::server::AppState;
use axum::{
    extract::{ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json, Router,
};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tower::ServiceBuilder;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
use tracing::{info, warn};

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: u16,
}

impl ErrorResponse {
    pub fn new(code: StatusCode, message: &str) -> (StatusCode, Json<Self>) {
        (
            code,
            Json(Self {
                error: message.to_string(),
                code: code.as_u16(),
            }),
        )
    }
}

pub fn apply(router: Router<Arc<AppState>>, _state: Arc<AppState>) -> Router<Arc<AppState>> {
    router.layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(CompressionLayer::new()),
    )
}

/// Network isolation middleware - checks if request IP is in allowed networks
pub async fn network_isolation(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    if !state.config.server.isolation_mode {
        return next.run(request).await;
    }

    let client_ip = addr.ip();
    let path = request.uri().path().to_string();
    let method = request.method().to_string();

    if is_ip_allowed(&client_ip, &state.config.server.allowed_networks) {
        next.run(request).await
    } else {
        // Log suspicious request
        log_suspicious_request(&client_ip.to_string(), &method, &path, "network_violation");

        ErrorResponse::new(StatusCode::FORBIDDEN, "Access denied: unauthorized network")
            .into_response()
    }
}

/// JWT authentication middleware
pub async fn jwt_auth(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();

    // Skip auth for health checks, root, and monitoring dashboard
    if path == "/health" || path == "/" || path == "/monitoring" {
        return next.run(request).await;
    }

    let auth_header = request
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok());

    let client_ip = addr.ip().to_string();
    let method = request.method().to_string();
    let path = path.to_string();

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            let jwt_manager = JwtManager::new(&state.config.auth.jwt_secret);

            match jwt_manager.validate_token(token) {
                Ok(claims) => {
                    info!(subject = %claims.sub, path = %path, "Authenticated request");
                    next.run(request).await
                }
                Err(e) => {
                    log_suspicious_request(
                        &client_ip,
                        &method,
                        &path,
                        &format!("invalid_jwt: {}", e),
                    );
                    ErrorResponse::new(StatusCode::UNAUTHORIZED, &e.to_string()).into_response()
                }
            }
        }
        Some(_) => {
            log_suspicious_request(&client_ip, &method, &path, "malformed_auth_header");
            ErrorResponse::new(StatusCode::UNAUTHORIZED, "Invalid authorization header")
                .into_response()
        }
        None => {
            log_suspicious_request(&client_ip, &method, &path, "missing_auth");
            ErrorResponse::new(StatusCode::UNAUTHORIZED, "Missing authorization token")
                .into_response()
        }
    }
}

/// Request timing middleware
pub async fn request_timing(request: Request, next: Next) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status();

    info!(
        method = %method,
        path = %path,
        status = %status.as_u16(),
        duration_ms = %duration.as_millis(),
        "Request completed"
    );

    response
}

/// Log suspicious requests to a separate target for security analysis
fn log_suspicious_request(ip: &str, method: &str, path: &str, reason: &str) {
    warn!(
        target: "suspicious",
        ip = %ip,
        method = %method,
        path = %path,
        reason = %reason,
        "Suspicious request detected"
    );
}

/// Rate limiting state
pub mod rate_limit {
    use std::collections::HashMap;
    use std::net::IpAddr;
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use tokio::sync::RwLock;

    #[derive(Clone)]
    pub struct RateLimiter {
        requests: Arc<RwLock<HashMap<IpAddr, Vec<Instant>>>>,
        max_requests: usize,
        window: Duration,
    }

    impl RateLimiter {
        pub fn new(max_requests: usize, window_secs: u64) -> Self {
            Self {
                requests: Arc::new(RwLock::new(HashMap::new())),
                max_requests,
                window: Duration::from_secs(window_secs),
            }
        }

        pub async fn check(&self, ip: IpAddr) -> bool {
            let now = Instant::now();
            let mut requests = self.requests.write().await;

            let entry = requests.entry(ip).or_insert_with(Vec::new);

            // Remove old requests outside the window
            entry.retain(|&t| now.duration_since(t) < self.window);

            if entry.len() >= self.max_requests {
                return false;
            }

            entry.push(now);
            true
        }

        /// Cleanup old entries periodically
        #[allow(dead_code)]
        pub async fn cleanup(&self) {
            let now = Instant::now();
            let mut requests = self.requests.write().await;

            requests.retain(|_, times| {
                times.retain(|&t| now.duration_since(t) < self.window);
                !times.is_empty()
            });
        }
    }

    impl Default for RateLimiter {
        fn default() -> Self {
            // 100 requests per minute by default
            Self::new(100, 60)
        }
    }
}

/// Rate limiting middleware
pub async fn rate_limiting(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let client_ip = addr.ip();

    if !state.rate_limiter.check(client_ip).await {
        let path = request.uri().path().to_string();
        let method = request.method().to_string();

        log_suspicious_request(
            &client_ip.to_string(),
            &method,
            &path,
            "rate_limit_exceeded",
        );

        return ErrorResponse::new(StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded")
            .into_response();
    }

    next.run(request).await
}
