mod server;

use std::{
    net::{IpAddr, TcpListener, UdpSocket},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

use server::{AppState, ReceivedFile, SharedFile, TextItem};

#[derive(Serialize)]
struct Info {
    url: String,
    qr: String,
    ip: String,
    port: u16,
    token: String,
    downloads_dir: String,
    received: Vec<ReceivedFile>,
    shared: Vec<SharedFile>,
    texts: Vec<TextItem>,
}

#[derive(Deserialize)]
struct BlobInput {
    name: String,
    bytes: Vec<u8>,
}

fn local_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|a| a.ip())
}

fn downloads_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("SHARKYYDROP_DOWNLOADS") {
        return PathBuf::from(d);
    }
    if let Some(d) = dirs::download_dir() {
        return d;
    }
    if let Some(home) = dirs::home_dir() {
        return home.join("Downloads");
    }
    std::env::temp_dir()
}

#[tauri::command]
fn get_info(state: State<'_, Arc<AppState>>) -> Info {
    let url = format!("http://{}:{}/t/{}", state.ip, state.port, state.token);
    Info {
        qr: server::make_qr(&url),
        url,
        ip: state.ip.clone(),
        port: state.port,
        token: state.token.clone(),
        downloads_dir: state.downloads_dir.to_string_lossy().to_string(),
        received: state.received.lock().unwrap().clone(),
        shared: state.shared.lock().unwrap().clone(),
        texts: state.texts.lock().unwrap().clone(),
    }
}

#[tauri::command]
fn add_files(
    state: State<'_, Arc<AppState>>,
    paths: Vec<String>,
) -> Result<Vec<SharedFile>, String> {
    let mut added = Vec::new();
    for p in paths {
        let path = PathBuf::from(&p);
        if path.is_file() {
            match state.add_shared_path(&path) {
                Ok(item) => added.push(item),
                Err(e) => return Err(format!("{}: {e}", path.display())),
            }
        }
    }
    Ok(added)
}

#[tauri::command]
fn share_blobs(
    state: State<'_, Arc<AppState>>,
    items: Vec<BlobInput>,
) -> Result<Vec<SharedFile>, String> {
    let mut added = Vec::new();
    for item in items {
        match state.add_shared_bytes(&item.name, &item.bytes) {
            Ok(f) => added.push(f),
            Err(e) => return Err(format!("{}: {e}", item.name)),
        }
    }
    Ok(added)
}

#[tauri::command]
fn open_path(state: State<'_, Arc<AppState>>, path: String) -> Result<(), String> {
    let allowed = state
        .received
        .lock()
        .unwrap()
        .iter()
        .any(|f| f.path == path)
        || state.shared.lock().unwrap().iter().any(|f| f.path == path);
    if !allowed {
        return Err("файл не из этой сессии".into());
    }
    open::that(path).map_err(|e| e.to_string())
}

#[tauri::command]
fn open_downloads(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let dir = state.downloads_dir.clone();
    let _ = std::fs::create_dir_all(&dir);
    open::that(dir).map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_received(state: State<'_, Arc<AppState>>) {
    state.received.lock().unwrap().clear();
    state.notify();
}

#[tauri::command]
fn clear_shared(state: State<'_, Arc<AppState>>) {
    state.shared.lock().unwrap().clear();
    state.notify();
}

#[tauri::command]
fn clear_texts(state: State<'_, Arc<AppState>>) {
    state.texts.lock().unwrap().clear();
    state.notify();
}

#[tauri::command]
fn send_text(state: State<'_, Arc<AppState>>, text: String) -> Result<(), String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("пусто".into());
    }
    let text: String = text.chars().take(20000).collect();
    state.texts.lock().unwrap().push(TextItem {
        text,
        at: server::now(),
        from: "pc".into(),
    });
    state.notify();
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let listener = TcpListener::bind(("0.0.0.0", 0))?;
            let port = listener.local_addr()?.port();
            let ip = local_ip()
                .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
                .to_string();
            let shared_dir = std::env::var_os("SHARKYYDROP_SHARED")
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::temp_dir().join("sharkyydrop-shared"));
            let _ = std::fs::remove_dir_all(&shared_dir);
            let _ = std::fs::create_dir_all(&shared_dir);

            let state = Arc::new(AppState {
                token: server::new_token(),
                port,
                ip,
                downloads_dir: downloads_dir(),
                shared_dir,
                received: Mutex::new(Vec::new()),
                shared: Mutex::new(Vec::new()),
                texts: Mutex::new(Vec::new()),
                app: Some(app.handle().clone()),
                seq: std::sync::atomic::AtomicU64::new(0),
            });

            server::spawn(listener, state.clone());
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_info,
            add_files,
            share_blobs,
            open_path,
            open_downloads,
            clear_received,
            clear_shared,
            clear_texts,
            send_text
        ])
        .run(tauri::generate_context!())
        .expect("не удалось запустить приложение");
}
