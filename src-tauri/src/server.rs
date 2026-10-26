use std::{
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    body::Body,
    extract::{Multipart, Path as UrlPath, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

const MOBILE_HTML: &str = include_str!("mobile.html");

#[derive(Clone, Serialize)]
pub struct SharedFile {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub path: String,
    pub at: u64,
}

#[derive(Clone, Serialize)]
pub struct ReceivedFile {
    pub name: String,
    pub size: u64,
    pub path: String,
    pub at: u64,
}

#[derive(Clone, Serialize)]
pub struct TextItem {
    pub text: String,
    pub at: u64,
    pub from: String,
}

pub struct AppState {
    pub token: String,
    pub port: u16,
    pub ip: String,
    pub downloads_dir: PathBuf,
    pub shared_dir: PathBuf,
    pub received: Mutex<Vec<ReceivedFile>>,
    pub shared: Mutex<Vec<SharedFile>>,
    pub texts: Mutex<Vec<TextItem>>,
    pub app: Option<AppHandle>,
    pub seq: AtomicU64,
}

impl AppState {
    pub fn add_shared_path(&self, src: &Path) -> std::io::Result<SharedFile> {
        let name = src
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());
        let name = sanitize(&name);
        std::fs::create_dir_all(&self.shared_dir)?;
        let id = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let dest = self.shared_dir.join(format!("{id}-{name}"));
        std::fs::copy(src, &dest)?;
        let size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
        let item = SharedFile {
            id,
            name,
            size,
            path: dest.to_string_lossy().to_string(),
            at: now(),
        };
        self.shared.lock().unwrap().push(item.clone());
        self.notify();
        Ok(item)
    }

    pub fn add_shared_bytes(&self, name: &str, bytes: &[u8]) -> std::io::Result<SharedFile> {
        std::fs::create_dir_all(&self.shared_dir)?;
        let name = sanitize(name);
        let id = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let dest = self.shared_dir.join(format!("{id}-{name}"));
        std::fs::write(&dest, bytes)?;
        let item = SharedFile {
            id,
            name,
            size: bytes.len() as u64,
            path: dest.to_string_lossy().to_string(),
            at: now(),
        };
        self.shared.lock().unwrap().push(item.clone());
        self.notify();
        Ok(item)
    }

    pub fn notify(&self) {
        if let Some(app) = &self.app {
            let _ = app.emit("changed", ());
        }
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn sanitize(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = base
        .chars()
        .filter(|c| !matches!(c, '\0' | '\r' | '\n' | '\t'))
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "file".to_string()
    } else {
        trimmed
    }
}

fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let p = Path::new(name);
    let stem = p
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file")
        .to_string();
    let ext = p
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    for i in 1..10000u32 {
        let cand = dir.join(format!("{stem} ({i}){ext}"));
        if !cand.exists() {
            return cand;
        }
    }
    dir.join(format!("{}-{}", now(), name))
}

fn content_disposition(name: &str) -> String {
    let ascii: String = name
        .chars()
        .map(|c| {
            if c.is_ascii() && c != '"' && c != '\\' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let mut encoded = String::new();
    for b in name.as_bytes() {
        if b.is_ascii_alphanumeric() || matches!(*b, b'-' | b'.' | b'_' | b'~') {
            encoded.push(*b as char);
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        ascii, encoded
    )
}

fn forbidden() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(serde_json::json!({"ok": false, "error": "неверный код доступа"})),
    )
        .into_response()
}

fn check(state: &AppState, token: &str) -> bool {
    token == state.token
}

async fn root(State(state): State<Arc<AppState>>) -> Html<String> {
    let link = format!("/t/{}", state.token);
    Html(format!(
        r#"<!doctype html><html lang="ru"><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Sharkyydrop</title>
<style>
body{{margin:0;min-height:100vh;display:grid;place-items:center;background:#0B1B1D;color:#EAF5F3;font-family:system-ui,-apple-system,Segoe UI,Roboto,sans-serif;text-align:center;padding:24px}}
.card{{max-width:420px;background:#122628;border:1px solid #244447;border-radius:24px;padding:36px 28px}}
h1{{margin:0 0 8px;font-size:26px}}p{{color:#8FADAA;line-height:1.5;margin:0 0 22px}}
a{{display:inline-block;background:linear-gradient(135deg,#35BDB3,#0B7A75);color:#04201F;font-weight:700;text-decoration:none;padding:14px 22px;border-radius:14px}}
</style>
<div class="card"><h1>Sharkyydrop на связи</h1>
<p>Откройте ссылку с экрана компьютера или отсканируйте QR-код.</p>
<a href="{link}">Открыть страницу</a></div></html>"#
    ))
}

async fn mobile(State(state): State<Arc<AppState>>, UrlPath(token): UrlPath<String>) -> Response {
    if !check(&state, &token) {
        return forbidden();
    }
    Html(MOBILE_HTML).into_response()
}

async fn upload(
    State(state): State<Arc<AppState>>,
    UrlPath(token): UrlPath<String>,
    mut mp: Multipart,
) -> Response {
    if !check(&state, &token) {
        return forbidden();
    }
    if let Err(e) = tokio::fs::create_dir_all(&state.downloads_dir).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        )
            .into_response();
    }

    let mut saved: Vec<ReceivedFile> = Vec::new();
    loop {
        let field = match mp.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"ok": false, "error": e.to_string()})),
                )
                    .into_response()
            }
        };
        let raw_name = field.file_name().unwrap_or("file").to_string();
        let name = sanitize(&raw_name);
        let dest = unique_path(&state.downloads_dir, &name);
        let mut file = match tokio::fs::File::create(&dest).await {
            Ok(f) => f,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({"ok": false, "error": e.to_string()})),
                )
                    .into_response()
            }
        };
        use tokio::io::AsyncWriteExt;
        let mut size = 0u64;
        let mut field = field;
        loop {
            match field.chunk().await {
                Ok(Some(chunk)) => {
                    if file.write_all(&chunk).await.is_err() {
                        break;
                    }
                    size += chunk.len() as u64;
                }
                Ok(None) => break,
                Err(_) => break,
            }
        }
        let _ = file.flush().await;
        let item = ReceivedFile {
            name,
            size,
            path: dest.to_string_lossy().to_string(),
            at: now(),
        };
        state.received.lock().unwrap().push(item.clone());
        if let Some(app) = &state.app {
            let _ = app.emit("received", &item);
        }
        saved.push(item);
    }
    state.notify();
    Json(serde_json::json!({"ok": true, "saved": saved})).into_response()
}

async fn list_files(
    State(state): State<Arc<AppState>>,
    UrlPath(token): UrlPath<String>,
) -> Response {
    if !check(&state, &token) {
        return forbidden();
    }
    let list = state.shared.lock().unwrap().clone();
    Json(list).into_response()
}

async fn download(
    State(state): State<Arc<AppState>>,
    UrlPath((token, id)): UrlPath<(String, u64)>,
) -> Response {
    if !check(&state, &token) {
        return forbidden();
    }
    let item = state
        .shared
        .lock()
        .unwrap()
        .iter()
        .find(|f| f.id == id)
        .cloned();
    let Some(item) = item else {
        return (StatusCode::NOT_FOUND, "файл не найден").into_response();
    };
    let bytes = match tokio::fs::read(&item.path).await {
        Ok(b) => b,
        Err(e) => {
            return (StatusCode::NOT_FOUND, e.to_string()).into_response();
        }
    };
    let mime = mime_guess::from_path(&item.name).first_or_octet_stream();
    let body = Body::from(bytes);
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CONTENT_DISPOSITION, content_disposition(&item.name))
        .header(header::CONTENT_LENGTH, item.size.to_string())
        .body(body)
        .unwrap()
}

async fn list_text(
    State(state): State<Arc<AppState>>,
    UrlPath(token): UrlPath<String>,
) -> Response {
    if !check(&state, &token) {
        return forbidden();
    }
    let list = state.texts.lock().unwrap().clone();
    Json(list).into_response()
}

async fn post_text(
    State(state): State<Arc<AppState>>,
    UrlPath(token): UrlPath<String>,
    body: String,
) -> Response {
    if !check(&state, &token) {
        return forbidden();
    }
    let text = body.trim().to_string();
    if text.is_empty() {
        return Json(serde_json::json!({"ok": false, "error": "пусто"})).into_response();
    }
    let text: String = text.chars().take(20000).collect();
    let item = TextItem {
        text,
        at: now(),
        from: "phone".to_string(),
    };
    state.texts.lock().unwrap().push(item.clone());
    if let Some(app) = &state.app {
        let _ = app.emit("text", &item);
    }
    state.notify();
    Json(serde_json::json!({"ok": true})).into_response()
}

fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(|| async { "ok" }))
        .route("/t/{token}", get(mobile))
        .route("/t/{token}/upload", post(upload))
        .route("/t/{token}/files", get(list_files))
        .route("/t/{token}/files/{id}", get(download))
        .route("/t/{token}/text", get(list_text).post(post_text))
        .with_state(state)
}

pub fn spawn(listener: TcpListener, state: Arc<AppState>) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(_) => return,
        };
        rt.block_on(async move {
            let _ = listener.set_nonblocking(true);
            let listener = match tokio::net::TcpListener::from_std(listener) {
                Ok(l) => l,
                Err(_) => return,
            };
            let _ = axum::serve(listener, router(state)).await;
        });
    });
}

pub fn make_qr(url: &str) -> String {
    match qrcode::QrCode::new(url.as_bytes()) {
        Ok(code) => code
            .render::<qrcode::render::svg::Color>()
            .min_dimensions(240, 240)
            .quiet_zone(true)
            .dark_color(qrcode::render::svg::Color("#0B2B2A"))
            .light_color(qrcode::render::svg::Color("#FFFFFF"))
            .build(),
        Err(_) => String::new(),
    }
}

pub fn new_token() -> String {
    const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    (0..8)
        .map(|_| ALPHABET[fastrand::usize(..ALPHABET.len())] as char)
        .collect()
}
