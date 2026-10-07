//! Versioned, ownership-explicit C ABI shared by the Flutter desktop runners.
use serde::Deserialize;
use serde_json::{json, Value};
use std::ffi::{c_char, CStr, CString};
use std::sync::Mutex;

#[path = "platform/common.rs"]
mod common;
#[cfg(target_os = "macos")]
#[path = "platform/mac/mod.rs"]
mod macos;
#[cfg(windows)]
#[path = "platform/win/mod.rs"]
mod windows;
#[cfg(windows)]
mod proxy;

static REQUEST_LOCK: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
struct Request {
    version: u32,
    command: String,
    #[serde(default)]
    payload: Value,
}

fn dispatch(request: &str) -> Result<Value, String> {
    let request: Request = serde_json::from_str(request).map_err(|e| format!("请求无效：{e}"))?;
    if request.version != 1 {
        return Err("原生接口版本不匹配，请重新构建应用。".into());
    }
    // Proxy discovery is read-only and can block on PAC/WPAD. It must never
    // hold the lock protecting settings and process launches.
    #[cfg(windows)]
    if request.command == "proxy_for_url" {
        return proxy::resolve(request.payload["url"].as_str().unwrap_or_default())
            .map(|proxy| json!({"proxy": proxy}));
    }
    // Reject unknown commands before initializing OS services or touching files.
    if !["bootstrap", "discover", "validate", "save", "launch", "check_launch", "repair_launch", "create_shortcut", "launch_dream_skin", "reapply_dream_skin"].contains(&request.command.as_str()) {
        return Err(format!("未知命令：{}", request.command));
    }
    let _guard = REQUEST_LOCK.lock().map_err(|_| "系统功能锁不可用，请重启启动器。".to_string())?;
    #[cfg(windows)]
    { windows::execute(&request.command, request.payload) }
    #[cfg(target_os = "macos")]
    { macos::execute(&request.command, request.payload) }
    #[cfg(not(any(windows, target_os = "macos")))]
    { Err("仅支持 Windows 和 macOS。".into()) }
}

fn response(request: &str) -> String {
    let result = std::panic::catch_unwind(|| dispatch(request));
    let envelope = match result {
        Ok(Ok(data)) => json!({"version":1,"ok":true,"data":data}),
        Ok(Err(error)) => json!({"version":1,"ok":false,"error":error}),
        Err(_) => json!({"version":1,"ok":false,"error":"系统操作异常，请重启启动器后重试。"}),
    };
    envelope.to_string()
}

#[no_mangle]
pub extern "C" fn launcher_abi_version() -> u32 { 1 }

/// Returns an owned UTF-8 JSON string. Free exactly once with launcher_free.
///
/// # Safety
/// A non-null request must point to a live NUL-terminated UTF-8 string for the
/// duration of this call. Invoke off the UI thread; calls can block for 45s+.
#[no_mangle]
pub unsafe extern "C" fn launcher_call(request: *const c_char) -> *mut c_char {
    let value = if request.is_null() {
        json!({"version":1,"ok":false,"error":"请求不能为空。"}).to_string()
    } else {
        match CStr::from_ptr(request).to_str() {
            Ok(request) => response(request),
            Err(_) => json!({"version":1,"ok":false,"error":"请求必须是 UTF-8。"}).to_string(),
        }
    };
    // JSON serialization escapes all embedded NULs.
    CString::new(value).expect("JSON contains no raw NUL").into_raw()
}

/// # Safety
/// pointer must be null or an unfreed pointer returned by launcher_call.
#[no_mangle]
pub unsafe extern "C" fn launcher_free(pointer: *mut c_char) {
    if !pointer.is_null() { drop(CString::from_raw(pointer)); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_requests_are_errors_without_platform_side_effects() {
        for request in ["not json", r#"{"version":2,"command":"bootstrap"}"#, r#"{"version":1,"command":"invalid"}"#] {
            let value: Value = serde_json::from_str(&response(request)).unwrap();
            assert_eq!(value["ok"], false);
            assert_eq!(value["version"], 1);
            assert!(!value["error"].as_str().unwrap().is_empty());
        }
    }

    #[test]
    fn ffi_round_trip_and_null_handling() {
        unsafe {
            for _ in 0..100 {
                let request = CString::new(r#"{"version":1,"command":"中文未知命令"}"#).unwrap();
                let pointer = launcher_call(request.as_ptr());
                let result: Value = serde_json::from_str(CStr::from_ptr(pointer).to_str().unwrap()).unwrap();
                assert!(result["error"].as_str().unwrap().contains("中文未知命令"));
                launcher_free(pointer);
            }
            let pointer = launcher_call(std::ptr::null());
            assert!(!pointer.is_null());
            launcher_free(pointer);
            launcher_free(std::ptr::null_mut());
        }
    }

    #[test]
    fn timezone_contract_preserves_posix_sign_and_bounds() {
        let mut settings = common::Settings::default();
        settings.mode = "offset".into();
        for (offset, expected) in [(-12,"Etc/GMT+12"),(0,"Etc/UTC"),(14,"Etc/GMT-14")] {
            settings.offset = offset;
            assert_eq!(common::tz(&settings).unwrap(), expected);
        }
        settings.offset = 15;
        assert!(common::tz(&settings).is_err());
        settings.mode = "zone".into();
        settings.zone_id = "not/a/zone".into();
        assert!(common::tz(&settings).is_err());
    }
}
