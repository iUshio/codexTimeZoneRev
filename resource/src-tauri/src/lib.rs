use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{path::PathBuf, process::Command};
use tauri::AppHandle;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const BACKEND_BYTES: &[u8] = include_bytes!("../resources/CodexTimeZoneBackend.exe");

fn backend_path() -> Result<PathBuf, String> {
    let runtime = data_directory()?.join(".runtime");
    std::fs::create_dir_all(&runtime).map_err(|e| format!("无法创建运行目录：{e}"))?;
    let backend = runtime.join("CodexTimeZoneBackend.exe");
    let current_length = std::fs::metadata(&backend).map(|item| item.len()).unwrap_or_default();
    if current_length != BACKEND_BYTES.len() as u64 {
        let temporary = runtime.join("CodexTimeZoneBackend.tmp");
        std::fs::write(&temporary, BACKEND_BYTES).map_err(|e| format!("无法释放系统功能后端：{e}"))?;
        if backend.exists() { std::fs::remove_file(&backend).map_err(|e| e.to_string())?; }
        std::fs::rename(temporary, &backend).map_err(|e| format!("无法更新系统功能后端：{e}"))?;
    }
    Ok(backend)
}

fn data_directory() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let directory = executable.parent().ok_or_else(|| "无法确定程序目录。".to_string())?.join("data");
    std::fs::create_dir_all(&directory).map_err(|e| format!("无法创建数据目录：{e}"))?;
    Ok(directory)
}

fn call_backend_sync(_app: AppHandle, command: String, payload: Value) -> Result<Value, String> {
    let request = json!({
        "command": command,
        "dataDirectory": data_directory()?.to_string_lossy(),
        "launcherPath": std::env::current_exe().map_err(|e| e.to_string())?.to_string_lossy(),
        "settings": payload.get("settings").cloned(),
        "path": payload.get("path").and_then(Value::as_str).unwrap_or_default()
    });
    let encoded = STANDARD.encode(serde_json::to_vec(&request).map_err(|e| e.to_string())?);
    let mut process = Command::new(backend_path()?);
    process.arg(encoded);
    #[cfg(windows)] process.creation_flags(0x08000000);
    let output = process.output().map_err(|e| format!("无法启动系统功能后端：{e}"))?;
    let text = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    let reply: Value = serde_json::from_str(&text).map_err(|e| format!("系统功能后端返回了无效数据：{e}"))?;
    if reply.get("success").and_then(Value::as_bool) == Some(true) {
        Ok(reply.get("data").cloned().unwrap_or(Value::Null))
    } else {
        Err(reply.get("message").and_then(Value::as_str).unwrap_or("系统功能执行失败。").to_string())
    }
}

#[tauri::command]
async fn backend(app: AppHandle, command: String, payload: Option<Value>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || call_backend_sync(app, command, payload.unwrap_or_else(|| json!({}))))
        .await.map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![backend])
        .run(tauri::generate_context!())
        .expect("error while running Tauri application");
}
