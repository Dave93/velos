use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

#[derive(Clone)]
pub struct ApiToken(pub Option<String>);

pub async fn auth_middleware(
    token: axum::extract::Extension<ApiToken>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, impl IntoResponse> {
    let ApiToken(expected) = token.0;

    // If no token configured, allow all requests
    let Some(expected_token) = expected else {
        return Ok(next.run(req).await);
    };

    // Allow WebSocket upgrade without auth header (token can be in query)
    if req.uri().path() == "/ws" {
        // Check query param ?token=xxx for WebSocket
        if let Some(query) = req.uri().query() {
            for pair in query.split('&') {
                if let Some(val) = pair.strip_prefix("token=") {
                    if val == expected_token {
                        return Ok(next.run(req).await);
                    }
                }
            }
        }
    }

    // Check Authorization: Bearer <token> header
    if let Some(auth) = req.headers().get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(bearer_token) = auth_str.strip_prefix("Bearer ") {
                if bearer_token == expected_token {
                    return Ok(next.run(req).await);
                }
            }
        }
    }

    Err((
        StatusCode::UNAUTHORIZED,
        axum::Json(serde_json::json!({"error": "unauthorized: invalid or missing api token"})),
    ))
}
