mod middleware;
mod routes;
mod websocket;

use axum::middleware as axum_mw;
use axum::{Extension, Router};
use tower_http::cors::{Any, CorsLayer};
use velos_core::VelosError;

pub async fn start_server(port: u16, api_token: Option<String>) -> Result<(), VelosError> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = Router::new()
        .merge(routes::router())
        .merge(websocket::router())
        .layer(axum_mw::from_fn(middleware::auth_middleware))
        .layer(Extension(middleware::ApiToken(api_token)))
        .layer(cors);

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(VelosError::Io)?;

    eprintln!("[velos-api] Listening on http://{addr}");

    axum::serve(listener, app).await.map_err(VelosError::Io)
}
