#[path = "../../src-tauri/src/api.rs"]
mod api;
#[path = "../../src-tauri/src/lines.rs"]
mod lines;
#[path = "../../src-tauri/src/logging.rs"]
mod logging;
#[path = "../../src-tauri/src/media.rs"]
mod media;
#[path = "../../src-tauri/src/net.rs"]
mod net;
#[path = "../../src-tauri/src/network_policy.rs"]
mod network_policy;
#[path = "../../src-tauri/src/playback_log.rs"]
mod playback_log;
mod public_media;
mod rpc;
#[path = "../../src-tauri/src/session.rs"]
mod session;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Form, Path, RawQuery, Request, State},
    http::{header, HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use rand::RngCore;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use tower_http::services::{ServeDir, ServeFile};
#[derive(Clone)]
struct AppState {
    api: Arc<api::Api>,
    guest_api: Arc<api::Api>,
    public: bool,
    media_grants: Arc<Mutex<public_media::MediaGrants>>,
    password_hash: [u8; 32],
    sessions: Arc<Mutex<HashMap<String, Instant>>>,
    attempts: Arc<Mutex<Vec<Instant>>>,
    secure: bool,
    origin: String,
}
fn token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|s| s.trim().strip_prefix("hg_session="))
}
fn authenticated(s: &AppState, h: &HeaderMap) -> bool {
    token(h).is_some_and(|t| {
        s.sessions
            .lock()
            .unwrap()
            .get(t)
            .is_some_and(|expiry| *expiry > Instant::now())
    })
}
async fn guard(State(s): State<AppState>, req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if !matches!(req.method().as_str(), "GET" | "HEAD") {
        let origin = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok());
        if origin.is_some_and(|v| v != s.origin) {
            return StatusCode::FORBIDDEN.into_response();
        }
        if path.starts_with("/api/")
            && req
                .headers()
                .get("X-HG-Request")
                .and_then(|v| v.to_str().ok())
                != Some("1")
        {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    let public_read = s.public
        && (path.starts_with("/media/")
            || (matches!(req.method().as_str(), "GET" | "HEAD")
                && path != "/logout"
                && !path.starts_with("/api/"))
            || path
                .strip_prefix("/api/invoke/")
                .is_some_and(public_command));
    if s.public && path.starts_with("/api/") && !public_read && !authenticated(&s, req.headers()) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error":"此操作仅管理员可用；公开网页版支持访客浏览与播放"})),
        )
            .into_response();
    }
    if path != "/login" && !public_read && !authenticated(&s, req.headers()) {
        return if path.starts_with("/api/") || path.starts_with("/media/") {
            (StatusCode::UNAUTHORIZED, Json(json!({"error":"请先登录"}))).into_response()
        } else {
            Redirect::to("/login").into_response()
        };
    }
    let mut r = next.run(req).await;
    r.headers_mut()
        .insert(header::CACHE_CONTROL, "private, no-store".parse().unwrap());
    r.headers_mut()
        .insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    r.headers_mut()
        .insert("Referrer-Policy", "same-origin".parse().unwrap());
    r.headers_mut()
        .insert("X-Frame-Options", "DENY".parse().unwrap());
    r
}
async fn login_page() -> Html<&'static str> {
    Html(include_str!("login.html"))
}
#[derive(Deserialize)]
struct Login {
    password: String,
}
async fn login(State(s): State<AppState>, Form(f): Form<Login>) -> Response {
    {
        let mut a = s.attempts.lock().unwrap();
        a.retain(|t| t.elapsed() < Duration::from_secs(60));
        if a.len() >= 10 {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                "尝试过于频繁，请一分钟后重试",
            )
                .into_response();
        }
        a.push(Instant::now());
    }
    let hash: [u8; 32] = Sha256::digest(f.password.as_bytes()).into();
    if !bool::from(hash.ct_eq(&s.password_hash)) {
        tokio::time::sleep(Duration::from_millis(500)).await;
        return (
            StatusCode::UNAUTHORIZED,
            Html("密码错误，<a href='/login'>返回登录</a>"),
        )
            .into_response();
    }
    let mut random = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut random);
    let value = random
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect::<String>();
    {
        let mut sessions = s.sessions.lock().unwrap();
        sessions.retain(|_, e| *e > Instant::now());
        if sessions.len() >= 100 {
            sessions.clear();
        }
        sessions.insert(value.clone(), Instant::now() + Duration::from_secs(86400));
    }
    let cookie = format!(
        "hg_session={value}; Path=/; HttpOnly; SameSite=Strict; Max-Age=86400{}",
        if s.secure { "; Secure" } else { "" }
    );
    ([(header::SET_COOKIE, cookie)], Redirect::to("/")).into_response()
}
async fn logout(State(s): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(t) = token(&headers) {
        s.sessions.lock().unwrap().remove(t);
    }
    (
        [(
            header::SET_COOKIE,
            "hg_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0",
        )],
        Redirect::to("/login"),
    )
        .into_response()
}
fn public_command(cmd: &str) -> bool {
    matches!(
        cmd,
        "app_boot"
            | "api_feed"
            | "api_category"
            | "api_rank"
            | "api_search"
            | "api_posts"
            | "api_post"
            | "api_video"
            | "api_play"
            | "api_comments"
            | "api_post_comments"
            | "line_state"
            | "debug_log"
    )
}
async fn invoke(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(cmd): Path<String>,
    Json(mut args): Json<serde_json::Value>,
) -> Response {
    let guest = s.public && !authenticated(&s, &headers);
    if guest && cmd == "app_boot" {
        return Json(json!({"user":null,"logged_in":false,"base":"","strict_security":true,"cli_relaxed":false,"public_access":true})).into_response();
    }
    if guest && cmd == "line_state" {
        return Json(json!({"base":"","default":"","fixed":[]})).into_response();
    }
    if guest {
        for key in ["size", "page"] {
            if let Some(n) = args.get(key).and_then(|v| v.as_u64()) {
                args[key] = json!(n.clamp(1, if key == "size" { 50 } else { 10000 }));
            }
        }
    }
    let api = if guest { &s.guest_api } else { &s.api };
    match rpc::dispatch(api, &cmd, args).await {
        Ok(mut v) => {
            if guest {
                s.media_grants.lock().unwrap().rewrite_api(&mut v, "");
            }
            Json(v).into_response()
        }
        Err(_) => (
            StatusCode::BAD_GATEWAY,
            Json(json!({"error":"请求失败，请稍后重试"})),
        )
            .into_response(),
    }
}
async fn proxy(
    State(s): State<AppState>,
    headers: HeaderMap,
    Path(route): Path<String>,
    RawQuery(query): RawQuery,
) -> Response {
    if !matches!(route.as_str(), "hls" | "seg" | "img") {
        return StatusCode::NOT_FOUND.into_response();
    }
    let query = query.unwrap_or_default();
    let params: HashMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    let guest = s.public && !authenticated(&s, &headers);
    let actual = if let Some(token) = params.get("t") {
        match s.media_grants.lock().unwrap().get(token, &route) {
            Some(target) => url::form_urlencoded::Serializer::new(String::new())
                .append_pair("u", &target)
                .finish(),
            None => return StatusCode::FORBIDDEN.into_response(),
        }
    } else if guest {
        return StatusCode::FORBIDDEN.into_response();
    } else {
        query
    };
    let mut response = media::handle(&format!("/{route}"), &actual, "/media").await;
    if guest && response.status().is_success() {
        if response.body().starts_with(b"#EXTM3U")
            || response.body().starts_with(b"\xef\xbb\xbf#EXTM3U")
        {
            if let Ok(text) = std::str::from_utf8(response.body()) {
                let replaced = s.media_grants.lock().unwrap().rewrite_playlist(text);
                *response.body_mut() = replaced.into_bytes();
                let length = response.body().len().to_string();
                response
                    .headers_mut()
                    .insert(header::CONTENT_LENGTH, length.parse().unwrap());
            }
        }
    } else if guest && !response.status().is_success() {
        return (StatusCode::BAD_GATEWAY, "媒体暂时不可用").into_response();
    }
    response.map(Body::from)
}
fn router(s: AppState, dist: PathBuf) -> Router {
    Router::new()
        .route("/login", get(login_page).post(login))
        .route("/logout", post(logout))
        .route("/api/invoke/{cmd}", post(invoke))
        .route("/media/{route}", get(proxy))
        .fallback_service(ServeDir::new(&dist).fallback(ServeFile::new(dist.join("index.html"))))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .layer(middleware::from_fn_with_state(s.clone(), guard))
        .with_state(s)
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let password = std::fs::read_to_string(
        std::env::var("HG_PASSWORD_FILE").expect("Set HG_PASSWORD_FILE to a private password file"),
    )?;
    let password = password.trim();
    if password.len() < 16 {
        return Err("Password must contain at least 16 characters".into());
    }
    let bind = std::env::var("HG_BIND").unwrap_or("127.0.0.1:8900".into());
    let origin = std::env::var("HG_ORIGIN").unwrap_or(format!("http://{bind}"));
    let data = PathBuf::from(std::env::var("HG_DATA_DIR").unwrap_or("web-server/data".into()));
    std::fs::create_dir_all(&data)?;
    net::bind_base(&data);
    network_policy::set_strict_security(true)?;
    playback_log::bind(&data);
    let s = AppState {
        api: Arc::new(api::Api::new()),
        guest_api: Arc::new(api::Api::new()),
        public: std::env::var("HG_PUBLIC_ACCESS").as_deref() == Ok("true"),
        media_grants: Default::default(),
        password_hash: Sha256::digest(password.as_bytes()).into(),
        sessions: Default::default(),
        attempts: Default::default(),
        secure: origin.starts_with("https://"),
        origin,
    };
    let app = router(
        s,
        PathBuf::from(std::env::var("HG_DIST").unwrap_or("dist-web".into())),
    );
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!("HGhub web listening on {bind}");
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod web_tests {
    use super::*;
    use tower::ServiceExt;
    fn state() -> AppState {
        AppState {
            api: Arc::new(api::Api::new()),
            guest_api: Arc::new(api::Api::new()),
            public: false,
            media_grants: Default::default(),
            password_hash: Sha256::digest(b"test-password-long-enough").into(),
            sessions: Default::default(),
            attempts: Default::default(),
            secure: false,
            origin: "http://localhost:8900".into(),
        }
    }
    #[tokio::test]
    async fn anonymous_cannot_read_app_api_or_media() {
        let app = router(state(), PathBuf::from("../dist-web"));
        for (path, status) in [
            ("/", 303),
            ("/assets/secret.js", 303),
            ("/media/hls?u=x", 401),
            ("/media/img?u=x", 401),
            ("/media/seg?u=x", 401),
        ] {
            let r = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(r.status().as_u16(), status, "{path}");
        }
        let r = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/invoke/api_feed")
                    .header("X-HG-Request", "1")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    }
    #[tokio::test]
    async fn login_cookie_origin_check_logout() {
        let s = state();
        let app = router(s.clone(), PathBuf::from("../dist-web"));
        let login = |password: &str| {
            Request::builder()
                .method("POST")
                .uri("/login")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(format!("password={password}")))
                .unwrap()
        };
        let bad = app.clone().oneshot(login("wrong")).await.unwrap();
        assert_eq!(bad.status(), StatusCode::UNAUTHORIZED);
        let good = app
            .clone()
            .oneshot(login("test-password-long-enough"))
            .await
            .unwrap();
        assert_eq!(good.status(), StatusCode::SEE_OTHER);
        let set_cookie = good.headers()[header::SET_COOKIE].to_str().unwrap();
        assert!(set_cookie.contains("HttpOnly"));
        assert!(set_cookie.contains("SameSite=Strict"));
        let cookie = set_cookie.split(';').next().unwrap();
        let denied = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/invoke/app_boot")
                    .header(header::COOKIE, cookie)
                    .header(header::ORIGIN, "https://evil.example")
                    .header("X-HG-Request", "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        let ok = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/invoke/app_boot")
                    .header(header::COOKIE, cookie)
                    .header("X-HG-Request", "1")
                    .header("content-type", "application/json")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ok.status(), StatusCode::OK);
        let out = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/logout")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(out.status(), StatusCode::SEE_OTHER);
        assert!(s.sessions.lock().unwrap().is_empty());
    }
    #[tokio::test]
    async fn public_visitors_can_boot_but_cannot_use_admin_or_arbitrary_proxy() {
        let mut s = state();
        s.public = true;
        let app = router(s, PathBuf::from("../dist-web"));
        let request = |cmd: &str| {
            Request::builder()
                .method("POST")
                .uri(format!("/api/invoke/{cmd}"))
                .header("X-HG-Request", "1")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap()
        };
        let boot = app.clone().oneshot(request("app_boot")).await.unwrap();
        assert_eq!(boot.status(), StatusCode::OK);
        let body = axum::body::to_bytes(boot.into_body(), 65536).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["public_access"], true);
        assert!(v["user"].is_null());
        for cmd in [
            "api_login",
            "api_register",
            "line_set",
            "security_set",
            "cache_clear",
            "net_diag",
        ] {
            assert_eq!(
                app.clone().oneshot(request(cmd)).await.unwrap().status(),
                StatusCode::FORBIDDEN,
                "{cmd}"
            );
        }
        for path in [
            "/media/img?u=https%3A%2F%2Fexample.com%2Fa.jpg",
            "/media/hls?t=not-issued",
        ] {
            let r = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::FORBIDDEN);
        }
    }
}
