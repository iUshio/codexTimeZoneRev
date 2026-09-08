use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{env, fs, io::{Read, Write}, net::{SocketAddr, TcpStream}, path::Path, process::{Command, Stdio}, time::{Duration, Instant}};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    #[serde(alias = "Mode")]
    pub mode: String,
    #[serde(alias = "ZoneId")]
    pub zone_id: String,
    #[serde(alias = "Offset")]
    pub offset: i32,
    #[serde(alias = "Executable")]
    pub executable: String,
    #[serde(alias = "DreamSkinCompatible")]
    pub dream_skin_compatible: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self { mode: "zone".into(), zone_id: "Asia/Shanghai".into(), offset: 8, executable: String::new(), dream_skin_compatible: false }
    }
}
pub fn zones() -> Value { serde_json::from_str(include_str!("zones.json")).expect("valid bundled zones") }
pub fn tz(s: &Settings) -> Result<String, String> {
    match s.mode.as_str() {
        "offset" if (-12..=14).contains(&s.offset) => Ok(if s.offset == 0 { "Etc/UTC".into() } else { format!("Etc/GMT{}{offset}", if s.offset > 0 { "-" } else { "+" }, offset = s.offset.abs()) }),
        "zone" if zones().as_array().unwrap().iter().any(|z| z["id"] == s.zone_id) => Ok(s.zone_id.clone()),
        _ => Err("请选择有效时区或 -12 至 +14 小时的 UTC 偏移。".into()),
    }
}
pub fn load(dir: &Path) -> Result<Settings, String> {
    let path = dir.join("settings.json");
    if !path.exists() { return Ok(Settings::default()); }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let s: Settings = serde_json::from_slice(bytes.strip_prefix(&[0xef,0xbb,0xbf]).unwrap_or(&bytes)).map_err(|e| format!("设置文件无效：{e}"))?;
    tz(&s)?; Ok(s)
}
pub fn save(dir: &Path, s: &Settings) -> Result<(), String> {
    tz(s)?; fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    temp.write_all(&serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(dir.join("settings.json")).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn settings(payload: &Value, dir: &Path) -> Result<Settings, String> {
    match payload.get("settings").filter(|s| !s.is_null()) {
        Some(s) => { let s = serde_json::from_value(s.clone()).map_err(|e| e.to_string())?; tz(&s)?; Ok(s) },
        None => load(dir),
    }
}

fn public_ip(use_proxy: bool) -> Result<Value, String> {
    let mut command = Command::new(if cfg!(windows) { "curl.exe" } else { "/usr/bin/curl" });
    command.args(["--fail", "--silent", "--show-error", "--max-time", "10", "https://ipinfo.io/json"]);
    if !use_proxy { command.env_remove("HTTP_PROXY").env_remove("HTTPS_PROXY").env_remove("ALL_PROXY").arg("--noproxy").arg("*"); }
    let output = command.output().map_err(|e| format!("无法查询公网 IP：{e}"))?;
    if !output.status.success() { return Err(format!("公网 IP 查询失败：{}", String::from_utf8_lossy(&output.stderr).trim())); }
    let value: Value = serde_json::from_slice(&output.stdout).map_err(|e| format!("公网 IP 服务返回无效数据：{e}"))?;
    let ip = value["ip"].as_str().filter(|s| !s.trim().is_empty()).ok_or("公网 IP 服务未返回地址。")?;
    let zone = value["timezone"].as_str().filter(|s| !s.trim().is_empty()).ok_or("公网 IP 服务未返回时区。")?;
    if !zones().as_array().unwrap().iter().any(|z| z["id"] == zone) { return Err(format!("公网 IP 返回了未收录的时区：{zone}")); }
    Ok(serde_json::json!({"ip":ip,"timezone":zone,"city":value["city"],"country":value["country_name"],"viaProxy":use_proxy}))
}
pub fn network_info() -> Result<Value, String> {
    let has_proxy = ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"].iter().any(|key| env::var_os(key).is_some_and(|v| !v.is_empty()));
    let proxy = if has_proxy { public_ip(true).ok() } else { None };
    let direct = public_ip(false).ok();
    if proxy.is_none() && direct.is_none() { return Err("无法获取公网 IP，请检查网络连接。".into()); }
    Ok(serde_json::json!({"proxy":proxy,"direct":direct,"proxyConfigured":has_proxy}))
}
// Read only a bounded loopback HTTP response, without proxies or redirects.
pub fn endpoint_ready(port: u16) -> bool {
    let read = || -> Option<()> {
        let addr = SocketAddr::from(([127,0,0,1], port));
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_millis(500)).ok()?;
        stream.set_read_timeout(Some(Duration::from_secs(1))).ok()?;
        stream.set_write_timeout(Some(Duration::from_secs(1))).ok()?;
        write!(stream, "GET /json/version HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n").ok()?;
        // Chromium keeps this connection alive even when asked to close it.
        // Stop at Content-Length instead of waiting for EOF and timing out.
        let mut reader = std::io::BufReader::new(stream);
        let mut bytes = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !bytes.ends_with(b"\r\n\r\n") {
            if bytes.len() >= 8192 || Instant::now() >= deadline { return None; }
            let mut byte = [0]; reader.read_exact(&mut byte).ok()?;
            bytes.push(byte[0]);
        }
        let header = std::str::from_utf8(&bytes).ok()?;
        if !header.starts_with("HTTP/1.1 200 ") && !header.starts_with("HTTP/1.0 200 ") { return None; }
        let mut length = None;
        for line in header.lines().skip(1) {
            if let Some((name, value)) = line.split_once(':') {
                if name.eq_ignore_ascii_case("transfer-encoding") { return None; }
                if name.eq_ignore_ascii_case("content-length") {
                    if length.is_some() { return None; }
                    length = Some(value.trim().parse::<usize>().ok()?);
                }
            }
        }
        let length = length.filter(|n| *n > 0 && *n <= 65536)?;
        let mut body = vec![0; length]; reader.read_exact(&mut body).ok()?;
        let v: Value = serde_json::from_slice(&body).ok()?;
        let socket = v["webSocketDebuggerUrl"].as_str()?;
        if ![format!("ws://127.0.0.1:{port}/"),format!("ws://localhost:{port}/"),format!("ws://[::1]:{port}/")].iter().any(|p| socket.starts_with(p)) { return None; }
        Some(())
    };
    read().is_some()
}
pub fn wait_endpoint(port: u16, timeout: Duration) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        if endpoint_ready(port) { return Ok(()); }
        if Instant::now() >= deadline { return Err(format!("客户端已启动，但皮肤调试接口 127.0.0.1:{port} 未就绪。请完全退出客户端后勾选兼容启动重试。")); }
        std::thread::sleep(Duration::from_millis(250));
    }
}
pub fn run_logged(mut command: Command, log_path: &Path, timeout: Duration) -> Result<(), String> {
    fs::create_dir_all(log_path.parent().ok_or("日志路径无效")?).map_err(|e| e.to_string())?;
    let log = fs::File::create(log_path).map_err(|e| e.to_string())?;
    let mut child = command.stdin(Stdio::null()).stdout(log.try_clone().map_err(|e| e.to_string())?).stderr(log).spawn().map_err(|e| e.to_string())?;
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if status.success() { return Ok(()); }
            let detail = fs::read_to_string(log_path).unwrap_or_default().lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
            return Err(format!("Dream Skin 应用失败（{status}）：\n{detail}\n日志：{}", log_path.display()));
        }
        if Instant::now() >= deadline {
            std::thread::spawn(move || { let _ = child.wait(); });
            return Err(format!("Dream Skin 验证超时，尚未确认皮肤生效，任务仍在后台运行。日志：{}", log_path.display()));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_settings_and_atomic_replace() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("settings.json"), "\u{feff}{\"Mode\":\"offset\",\"Offset\":-5,\"DreamSkinCompatible\":true}").unwrap();
        let mut s = load(dir.path()).unwrap();
        assert_eq!(tz(&s).unwrap(), "Etc/GMT+5");
        assert!(s.dream_skin_compatible);
        s.offset = 9;
        save(dir.path(), &s).unwrap();
        assert_eq!(load(dir.path()).unwrap().offset, 9);
        s.offset = 99;
        assert!(save(dir.path(), &s).is_err());
        assert_eq!(load(dir.path()).unwrap().offset, 9);
    }
    #[test]
    fn debugger_endpoint_rejects_unrelated_http_service() {
        for valid in [false, true] {
            let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let port = listener.local_addr().unwrap().port();
            let (done, wait) = std::sync::mpsc::channel();
            let server = std::thread::spawn(move || {
                let (mut connection, _) = listener.accept().unwrap();
                connection.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                // TCP may split one HTTP request across several reads. Closing
                // with unread request bytes can reset the connection on macOS.
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    connection.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                    assert!(request.len() < 4096);
                }
                let host = if valid { "127.0.0.1" } else { "example.invalid" };
                let body = format!("{{\"webSocketDebuggerUrl\":\"ws://{host}:{port}/devtools/browser/test\"}}");
                write!(connection, "HTTP/1.1 200 OK\r\nContent-Length:{}\r\n\r\n{body}", body.len()).unwrap();
                // Model Chromium: keep the socket open until the probe returns.
                let _ = wait.recv_timeout(Duration::from_secs(3));
            });
            assert_eq!(endpoint_ready(port), valid);
            done.send(()).unwrap();
            server.join().unwrap();
        }
    }
    #[cfg(unix)]
    #[test]
    fn skin_script_failure_is_not_reported_as_success() {
        let dir = tempfile::tempdir().unwrap();
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "echo verification-failed; exit 7"]);
        let error = run_logged(command, &dir.path().join("skin.log"), Duration::from_secs(5)).unwrap_err();
        assert!(error.contains("verification-failed"));
        assert!(error.contains("7"));
    }
}
