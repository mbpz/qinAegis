//! Loopback transport boundary. Business logic never receives session credentials.
use crate::{app::AppState, assets, dispatch};
use axum::{
    extract::{DefaultBodyLimit, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::sync::{watch, Semaphore};

const COOKIE: &str = "qinaegis_session";
const SESSION_TTL: Duration = Duration::from_secs(8 * 3600);
const MAX_SESSIONS: usize = 64;

struct Session {
    csrf: String,
    touched: Instant,
}
#[derive(Clone)]
pub struct ServerState {
    pub app: Arc<Mutex<AppState>>,
    pub origin: String,
    pub instance_id: String,
    owner_secret: String,
    grants: Arc<Mutex<HashMap<String, Instant>>>,
    sessions: Arc<Mutex<HashMap<String, Session>>>,
    requests: Arc<Semaphore>,
    blocking_work: Arc<Semaphore>,
    stopping: Arc<AtomicBool>,
    shutdown: watch::Sender<bool>,
}

impl ServerState {
    pub fn new(
        app: AppState,
        origin: String,
        instance_id: String,
        owner_secret: String,
        shutdown: watch::Sender<bool>,
    ) -> Self {
        Self {
            app: Arc::new(Mutex::new(app)),
            origin,
            instance_id,
            owner_secret,
            grants: Arc::default(),
            sessions: Arc::default(),
            requests: Arc::new(Semaphore::new(32)),
            blocking_work: Arc::new(Semaphore::new(8)),
            stopping: Arc::new(AtomicBool::new(false)),
            shutdown,
        }
    }
    pub fn launch_token(&self) -> anyhow::Result<String> {
        let token = random_token()?;
        let mut grants = self
            .grants
            .lock()
            .map_err(|_| anyhow::anyhow!("session unavailable"))?;
        grants.retain(|_, expiry| *expiry > Instant::now());
        anyhow::ensure!(
            grants.len() < MAX_SESSIONS,
            "too many pending browser launches"
        );
        grants.insert(token.clone(), Instant::now() + Duration::from_secs(60));
        Ok(token)
    }
    pub fn begin_shutdown(&self) -> Result<(), &'static str> {
        let app = self
            .app
            .try_lock()
            .map_err(|_| "Application is busy; retry shortly")?;
        if app.active_workers.load(Ordering::Acquire) != 0 {
            return Err("Wait for running and cancelling tasks to finish before shutting down");
        }
        self.stopping.store(true, Ordering::Release);
        Ok(())
    }
    fn local_owner(&self, headers: &HeaderMap) -> bool {
        !headers.contains_key(header::ORIGIN)
            && headers
                .get("x-qinaegis-owner")
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| secret_eq(v, &self.owner_secret))
    }
    fn csrf(&self, headers: &HeaderMap) -> Option<String> {
        let sid = headers
            .get(header::COOKIE)?
            .to_str()
            .ok()?
            .split(';')
            .find_map(|s| s.trim().strip_prefix("qinaegis_session="))?;
        let mut sessions = self.sessions.lock().ok()?;
        sessions.retain(|_, s| s.touched.elapsed() < SESSION_TTL);
        let session = sessions.get_mut(sid)?;
        session.touched = Instant::now();
        Some(session.csrf.clone())
    }
}

/// OS entropy only. Failure prevents startup rather than falling back to timestamps.
pub fn random_token() -> anyhow::Result<String> {
    #[cfg(unix)]
    {
        use std::io::Read;
        let mut bytes = [0u8; 32];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
        Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
    }
    #[cfg(windows)]
    {
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", "$b = New-Object byte[] 32; [System.Security.Cryptography.RandomNumberGenerator]::Create().GetBytes($b); [BitConverter]::ToString($b).Replace('-', '').ToLowerInvariant()"])
            .output()?;
        let value = String::from_utf8(output.stdout)?.trim().to_owned();
        anyhow::ensure!(
            output.status.success()
                && value.len() == 64
                && value.bytes().all(|b| b.is_ascii_hexdigit()),
            "system entropy unavailable"
        );
        Ok(value)
    }
    #[cfg(not(any(unix, windows)))]
    {
        anyhow::bail!("unsupported operating system")
    }
}
fn secret_eq(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |v, (a, b)| v | (a ^ b)) == 0
}
fn failure(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({"ok":false,"error":{"code":code,"message":message}})),
    )
        .into_response()
}

pub fn router(state: ServerState) -> Router {
    Router::new()
        .route("/healthz", get(health))
        .route("/api/v2/session", get(session).post(bootstrap))
        .route("/api/v2/launch", post(launch))
        .route("/api/v2/shutdown", post(shutdown))
        .route("/api/v2/rpc", post(rpc))
        .route("/api/v2/runtime-upload/{id}/{offset}", put(runtime_upload))
        .route("/api/v2/allure/{project}/{run_id}", get(allure_report))
        .fallback(static_asset)
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .layer(middleware::from_fn_with_state(state.clone(), boundary))
        .with_state(state)
}
async fn boundary(State(state): State<ServerState>, request: Request, next: Next) -> Response {
    let expected_host = state.origin.strip_prefix("http://").unwrap_or("");
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|v| v.to_str().ok());
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    if host != Some(expected_host)
        || (origin.is_some() && origin != Some(state.origin.as_str()))
        || request
            .headers()
            .get("sec-fetch-site")
            .is_some_and(|v| v == "cross-site")
    {
        return failure(
            StatusCode::FORBIDDEN,
            "ORIGIN_REJECTED",
            "Only this local workbench origin is allowed",
        );
    }
    let Ok(_permit) = state.requests.try_acquire() else {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "BUSY",
            "Too many active requests",
        );
    };
    let mut response = match tokio::time::timeout(Duration::from_secs(60), next.run(request)).await
    {
        Ok(response) => response,
        Err(_) => failure(StatusCode::GATEWAY_TIMEOUT, "TIMEOUT", "Request timed out"),
    };
    let headers = response.headers_mut();
    for (key, value) in [
        ("cache-control", "no-store"), ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"), ("x-frame-options", "DENY"),
        ("content-security-policy", "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; frame-src 'self' blob:; frame-ancestors 'none'; base-uri 'none'; form-action 'self'"),
    ] { if !headers.contains_key(key) { headers.insert(key, HeaderValue::from_static(value)); } }
    response
}

// A navigated document gets its own restrictive CSP. srcdoc/blob reports inherit
// the workbench's script-src 'self', which blocks Allure's inline bootstrap.
async fn allure_report(State(state): State<ServerState>, request: Request) -> Response {
    if state.csrf(request.headers()).is_none() {
        return failure(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "Valid local session required");
    }
    let parts: Vec<_> = request.uri().path().split('/').collect();
    let (Some(project), Some(run_id)) = (parts.get(4), parts.get(5)) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(permit) = state.blocking_work.clone().try_acquire_owned() else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, "BUSY", "Application work queue is full");
    };
    let (project, run_id) = (project.to_string(), run_id.to_string());
    match tokio::task::spawn_blocking(move || {
        let _permit = permit;
        dispatch::validate_report_path(&project, &run_id)
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        AppState::allure_html(&project, &run_id).map_err(|_| StatusCode::NOT_FOUND)
    }).await {
        Ok(Ok((html, _))) => ([
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::X_FRAME_OPTIONS, "SAMEORIGIN"),
            (header::CONTENT_SECURITY_POLICY, "sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline' blob: data:; style-src 'unsafe-inline' blob: data:; img-src data: blob:; font-src data: blob:; connect-src blob: data:; worker-src blob:; frame-ancestors 'self'; base-uri 'none'; form-action 'none'"),
        ], html).into_response(),
        Ok(Err(status)) => failure(status, "REPORT_UNAVAILABLE", "Allure report unavailable"),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "REPORT_UNAVAILABLE", "Allure report unavailable"),
    }
}
async fn health(State(state): State<ServerState>) -> Json<Value> {
    Json(
        json!({"product":"qinaegis","api":2,"ready":true,"version":crate::app::APP_VERSION,"instance_id":state.instance_id}),
    )
}
async fn launch(State(state): State<ServerState>, headers: HeaderMap) -> Response {
    if !state.local_owner(&headers) {
        return failure(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "Use the local launcher",
        );
    }
    match state.launch_token() {
        Ok(token) => Json(json!({"token":token})).into_response(),
        Err(_) => failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "SESSION_UNAVAILABLE",
            "Cannot create browser session",
        ),
    }
}
#[derive(Deserialize)]
struct Bootstrap {
    token: String,
}
async fn bootstrap(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(body): Json<Bootstrap>,
) -> Response {
    if headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(state.origin.as_str()) {
        return failure(
            StatusCode::FORBIDDEN,
            "ORIGIN_REJECTED",
            "A local browser origin is required",
        );
    }
    let Ok(mut grants) = state.grants.lock() else {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "SESSION_UNAVAILABLE",
            "Session unavailable",
        );
    };
    let valid = grants
        .remove(&body.token)
        .is_some_and(|expiry| expiry > Instant::now());
    drop(grants);
    if !valid {
        return failure(
            StatusCode::UNAUTHORIZED,
            "INVALID_TOKEN",
            "Launch authorization expired; reopen using qinaegis open",
        );
    }
    let (Ok(sid), Ok(csrf)) = (random_token(), random_token()) else {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "SESSION_UNAVAILABLE",
            "System entropy unavailable",
        );
    };
    let Ok(mut sessions) = state.sessions.lock() else {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "SESSION_UNAVAILABLE",
            "Session unavailable",
        );
    };
    sessions.retain(|_, session| session.touched.elapsed() < SESSION_TTL);
    if sessions.len() >= MAX_SESSIONS {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "SESSION_LIMIT",
            "Too many browser sessions",
        );
    }
    sessions.insert(
        sid.clone(),
        Session {
            csrf: csrf.clone(),
            touched: Instant::now(),
        },
    );
    let mut response = Json(json!({"csrf":csrf})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{COOKIE}={sid}; HttpOnly; SameSite=Strict; Path=/; Max-Age=28800"
        ))
        .expect("hex cookie"),
    );
    response
}
async fn session(State(state): State<ServerState>, headers: HeaderMap) -> Response {
    match state.csrf(&headers) {
        Some(csrf) => Json(json!({"csrf":csrf})).into_response(),
        None => failure(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "Reopen this workbench using qinaegis open",
        ),
    }
}
#[derive(Deserialize)]
struct RpcRequest {
    id: Value,
    method: String,
    #[serde(default = "empty_params")]
    params: Value,
}
fn empty_params() -> Value {
    json!({})
}
fn authorized_mutation(state: &ServerState, headers: &HeaderMap) -> bool {
    headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) == Some(state.origin.as_str())
        && state.csrf(headers).is_some_and(|csrf| headers.get("x-qinaegis-csrf")
            .and_then(|v| v.to_str().ok()).is_some_and(|v| secret_eq(v, &csrf)))
}
async fn runtime_upload(State(state): State<ServerState>, request: Request) -> Response {
    if !authorized_mutation(&state, request.headers()) {
        return failure(StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "Valid local session and CSRF token required");
    }
    let parts: Vec<_> = request.uri().path().split('/').collect();
    let id = parts.get(4).copied().unwrap_or("").to_owned();
    let offset = parts.get(5).and_then(|s| s.parse::<u64>().ok());
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) || offset.is_none() {
        return failure(StatusCode::BAD_REQUEST, "INVALID_UPLOAD", "Invalid upload identity or offset");
    }
    let bytes = match axum::body::to_bytes(request.into_body(), qin_aegis_core::runtime_transfer::CHUNK_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => return failure(StatusCode::PAYLOAD_TOO_LARGE, "CHUNK_LIMIT", "Runtime chunks must not exceed 512 KiB"),
    };
    let Ok(permit) = state.blocking_work.clone().try_acquire_owned() else {
        return failure(StatusCode::SERVICE_UNAVAILABLE, "BUSY", "Application work queue is full");
    };
    match tokio::task::spawn_blocking(move || -> anyhow::Result<u64> {
        let _permit = permit;
        let app = state.app.lock().map_err(|_| anyhow::anyhow!("Application state unavailable"))?;
        anyhow::ensure!(!state.stopping.load(Ordering::Acquire), "Service is shutting down");
        anyhow::ensure!(!app.runtime_maintenance_active(), "Wait for runtime maintenance cleanup before uploading");
        crate::runtime_api::transfers()?.append(&id, offset.unwrap(), &bytes)
    }).await {
        Ok(Ok(received)) => Json(json!({"received":received})).into_response(),
        Ok(Err(error)) => failure(StatusCode::CONFLICT, "UPLOAD_REJECTED", &error.to_string()),
        Err(_) => failure(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "Upload operation failed"),
    }
}
async fn rpc(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(body): Json<RpcRequest>,
) -> Response {
    let authorized = authorized_mutation(&state, &headers);
    if !authorized {
        return failure(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "Valid local session and CSRF token required",
        );
    }
    if !body.id.is_string() || !body.params.is_object() || body.method.len() > 128 {
        return failure(
            StatusCode::BAD_REQUEST,
            "INVALID_REQUEST",
            "Expected string id, method and object params",
        );
    }
    let id = body.id;
    let Ok(permit) = state.blocking_work.clone().try_acquire_owned() else {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "BUSY",
            "Application work queue is full",
        );
    };
    let result = tokio::task::spawn_blocking(move || {
        // Dropping an HTTP future does not cancel its blocking operation.
        let _permit = permit;
        let mut app = state
            .app
            .lock()
            .map_err(|_| "Application state unavailable")?;
        if state.stopping.load(Ordering::Acquire) {
            return Err("Service is shutting down");
        }
        Ok::<_, &str>(dispatch::dispatch(&mut app, &body.method, body.params))
    })
    .await;
    match result {
        Ok(Ok(data)) if data.get("ok") != Some(&Value::Bool(false)) => Json(json!({"id":id,"ok":true,"data":data})).into_response(),
        Ok(Ok(data)) => Json(json!({"id":id,"ok":false,"error":{"code":"RPC_ERROR","message":data.get("error").and_then(Value::as_str).unwrap_or("Operation failed")}})).into_response(),
        Ok(Err(message)) => failure(StatusCode::SERVICE_UNAVAILABLE,"UNAVAILABLE",message),
        _ => failure(StatusCode::INTERNAL_SERVER_ERROR,"INTERNAL","Application operation failed"),
    }
}
async fn shutdown(State(state): State<ServerState>, headers: HeaderMap) -> Response {
    if !state.local_owner(&headers) {
        return failure(
            StatusCode::UNAUTHORIZED,
            "UNAUTHORIZED",
            "Use qinaegis stop",
        );
    }
    if let Err(message) = state.begin_shutdown() {
        return failure(StatusCode::CONFLICT, "BUSY", message);
    }
    let _ = state.shutdown.send(true);
    Json(json!({"ok":true})).into_response()
}
async fn static_asset(request: Request) -> Response {
    if request.method() != axum::http::Method::GET {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    match assets::get(request.uri().path()) {
        Some((content, mime)) => ([(header::CONTENT_TYPE, mime)], content).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn loopback_session_boundary_and_rpc_survive_refresh() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let (tx, _rx) = watch::channel(false);
        let app = AppState {
            config: None,
            ..AppState::default()
        };
        let state = ServerState::new(
            app,
            origin.clone(),
            "test-instance".into(),
            random_token().unwrap(),
            tx,
        );
        let grant = state.launch_token().unwrap();
        let app = router(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let client = reqwest::Client::new();
        assert_eq!(
            client
                .get(format!("{origin}/healthz"))
                .header("Host", "evil.invalid")
                .send()
                .await
                .unwrap()
                .status(),
            403
        );
        assert_eq!(
            client
                .get(format!("{origin}/missing.js"))
                .send()
                .await
                .unwrap()
                .status(),
            404
        );
        assert_eq!(
            client
                .get(format!("{origin}/api/v2/rpc"))
                .send()
                .await
                .unwrap()
                .status(),
            405
        );
        let response = client
            .post(format!("{origin}/api/v2/session"))
            .header("Origin", &origin)
            .json(&json!({"token":grant}))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());
        let cookie = response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let csrf = response.json::<Value>().await.unwrap()["csrf"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(
            client
                .post(format!("{origin}/api/v2/session"))
                .header("Origin", &origin)
                .json(&json!({"token":grant}))
                .send()
                .await
                .unwrap()
                .status(),
            401
        );
        let refreshed = client
            .get(format!("{origin}/api/v2/session"))
            .header("Cookie", &cookie)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
        assert_eq!(refreshed["csrf"], csrf);
        for (supplied_origin, supplied_csrf, expected) in [
            (origin.as_str(), "bad", 401),
            ("https://evil.invalid", csrf.as_str(), 403),
            (origin.as_str(), csrf.as_str(), 200),
        ] {
            let response = client
                .post(format!("{origin}/api/v2/rpc"))
                .header("Origin", supplied_origin)
                .header("Cookie", &cookie)
                .header("x-qinaegis-csrf", supplied_csrf)
                .json(&json!({"id":"r1","method":"checkConfig","params":{}}))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), expected);
            if expected == 200 {
                let body = response.json::<Value>().await.unwrap();
                assert_eq!(body["id"], "r1");
                assert_eq!(body["data"]["configured"], false);
            }
        }
        state
            .app
            .lock()
            .unwrap()
            .active_workers
            .store(1, Ordering::Release);
        assert_eq!(
            client
                .post(format!("{origin}/api/v2/shutdown"))
                .header("x-qinaegis-owner", &state.owner_secret)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::CONFLICT
        );
        assert!(!state.stopping.load(Ordering::Acquire));
        state
            .app
            .lock()
            .unwrap()
            .active_workers
            .store(0, Ordering::Release);
        assert_eq!(
            client
                .post(format!("{origin}/api/v2/shutdown"))
                .header("x-qinaegis-owner", &state.owner_secret)
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::OK
        );
        assert_eq!(
            client
                .post(format!("{origin}/api/v2/rpc"))
                .header("Origin", &origin)
                .header("Cookie", &cookie)
                .header("x-qinaegis-csrf", &csrf)
                .json(&json!({"id":"late","method":"checkConfig","params":{}}))
                .send()
                .await
                .unwrap()
                .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        task.abort();
    }
}
