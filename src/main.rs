use std::{net::SocketAddr, sync::LazyLock, time::Instant};

use axum::{
    Router,
    http::{HeaderName, HeaderValue, header},
    routing::get,
};
use tokio::{net::TcpListener, signal};

/// 이미지 태그. CI 가 Docker build-arg 로 넘긴다
const VERSION: &str = match option_env!("GIT_SHA") {
    Some(v) => v,
    None => "dev",
};

/// 파드 이름. 쿠버네티스가 UTS hostname 을 파드 이름으로 맞춰 주므로
/// Downward API 로 매니페스트를 고치지 않고 procfs 에서 읽는다
static POD: LazyLock<String> = LazyLock::new(|| {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .map(|s| s.trim().to_owned())
        .unwrap_or_else(|_| "unknown".to_owned())
});

static STARTED: LazyLock<Instant> = LazyLock::new(Instant::now);

/// 어느 파드가 어느 버전으로 답했는지 알리는 헤더. 롤링 배포 중 구버전과
/// 신버전 응답 비율을 세는 데 쓴다
static IDENT: LazyLock<[(HeaderName, HeaderValue); 2]> = LazyLock::new(|| {
    [
        (
            HeaderName::from_static("x-pod"),
            HeaderValue::from_str(POD.as_str())
                .unwrap_or_else(|_| HeaderValue::from_static("unknown")),
        ),
        (
            HeaderName::from_static("x-version"),
            HeaderValue::from_str(VERSION).unwrap_or_else(|_| HeaderValue::from_static("unknown")),
        ),
    ]
});

async fn root() -> ([(HeaderName, HeaderValue); 2], &'static str) {
    (IDENT.clone(), "pong")
}

async fn whoami() -> ([(HeaderName, HeaderValue); 3], String) {
    let [pod, version] = IDENT.clone();
    let body = format!(
        r#"{{"pod":"{}","version":"{}","uptimeSeconds":{}}}"#,
        POD.as_str(),
        VERSION,
        STARTED.elapsed().as_secs()
    );
    let content_type = (
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    ([pod, version, content_type], body)
}

fn app() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/whoami", get(whoami))
        // liveness 와 readiness 를 나눠 둔다. 같은 경로를 쓰면 "살아 있지만
        // 아직 받을 수 없다" 를 표현할 수 없고, 의존성 장애가 전체 재시작으로 번진다
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(|| async { "ok" }))
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
    // 첫 요청이 아니라 기동 시점을 uptime 기준으로 삼는다
    LazyLock::force(&STARTED);
    println!("listening on {addr} pod={} version={VERSION}", POD.as_str());
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

    async fn get(path: &str) -> (StatusCode, Vec<(String, String)>, String) {
        let res = app()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = res.status();
        let headers = res
            .headers()
            .iter()
            .map(|(k, v)| (k.as_str().to_owned(), v.to_str().unwrap().to_owned()))
            .collect();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        (status, headers, String::from_utf8(body.to_vec()).unwrap())
    }

    fn header(headers: &[(String, String)], name: &str) -> Option<String> {
        headers
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    }

    #[tokio::test]
    async fn root_returns_pong() {
        let (status, _, body) = get("/").await;
        assert_eq!((status, body.as_str()), (StatusCode::OK, "pong"));
    }

    #[tokio::test]
    async fn root_carries_identity_headers() {
        let (_, headers, _) = get("/").await;
        assert!(header(&headers, "x-pod").is_some_and(|v| !v.is_empty()));
        assert!(header(&headers, "x-version").is_some_and(|v| !v.is_empty()));
    }

    #[tokio::test]
    async fn probes_return_ok() {
        assert_eq!(get("/healthz").await.2, "ok");
        assert_eq!(get("/readyz").await.2, "ok");
    }

    #[tokio::test]
    async fn whoami_returns_json() {
        let (status, headers, body) = get("/whoami").await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            header(&headers, "content-type").as_deref(),
            Some("application/json")
        );
        assert!(body.starts_with(r#"{"pod":"#));
        assert!(body.contains(r#""version":"#));
        assert!(body.contains(r#""uptimeSeconds":"#));
    }

    #[tokio::test]
    async fn unknown_path_is_404() {
        assert_eq!(get("/nope").await.0, StatusCode::NOT_FOUND);
    }
}
