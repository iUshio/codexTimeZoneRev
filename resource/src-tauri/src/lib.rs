use serde_json::{json, Value};
use tauri::AppHandle;

#[path = "../../backend/common.rs"]
mod common;
#[cfg(target_os = "macos")]
#[path = "../../backend/mac/mod.rs"]
mod macos;
#[cfg(windows)]
#[path = "../../backend/win/mod.rs"]
mod windows;

fn call_backend_sync(_app: AppHandle, command: String, payload: Value) -> Result<Value, String> {
    static REQUEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = REQUEST_LOCK.lock().map_err(|_| "系统功能锁不可用。".to_string())?;
    #[cfg(target_os = "macos")]
    { macos::execute(&command, payload) }
    #[cfg(windows)]
    { windows::execute(&command, payload) }
}

#[tauri::command]
async fn backend(app: AppHandle, command: String, payload: Option<Value>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        call_backend_sync(app, command, payload.unwrap_or_else(|| json!({})))
    }).await.map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![backend])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
