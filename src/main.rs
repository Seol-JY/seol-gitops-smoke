use std::net::SocketAddr;

use axum::{Router, routing::get};
use tokio::{net::TcpListener, signal};

fn app() -> Router {
    Router::new()
        .route("/", get(|| async { "pong" }))
        .route("/healthz", get(|| async { "ok" }))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c().await.expect("ctrl-c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("sigterm handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await.expect("bind");
    println!("listening on {addr}");
    axum::serve(listener, app())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("server");
}

#[cfg(test)]
mod tests {
    use super::app;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn get(path: &str) -> (StatusCode, String) {
        let res = app()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn root_returns_pong() {
        assert_eq!(get("/").await, (StatusCode::OK, "pong".to_string()));
    }

    #[tokio::test]
    async fn healthz_returns_ok() {
        assert_eq!(get("/healthz").await, (StatusCode::OK, "ok".to_string()));
    }

    #[tokio::test]
    async fn unknown_path_is_404() {
        assert_eq!(get("/nope").await.0, StatusCode::NOT_FOUND);
    }
}
