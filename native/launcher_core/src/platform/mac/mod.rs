use crate::common::{Settings, zones, tz, load, save, endpoint_ready, wait_endpoint, run_logged, network_info};
use serde_json::{json, Value};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("无法确定用户主目录。".into())
}
fn data_directory() -> Result<PathBuf, String> {
    Ok(home()?.join("Library/Application Support/com.fei-away.codextimezone"))
}
fn plist(bundle: &Path, key: &str) -> Result<String, String> {
    let result = Command::new("/usr/bin/plutil")
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(bundle.join("Contents/Info.plist"))
        .output()
        .map_err(|e| e.to_string())?;
    if !result.status.success() {
        return Err(format!("无法读取应用信息：{key}"));
    }
    Ok(String::from_utf8_lossy(&result.stdout).trim().into())
}
fn validate(path: &str) -> Result<(PathBuf, PathBuf), String> {
    let path = fs::canonicalize(path.trim().trim_matches('"'))
        .map_err(|_| "客户端路径不存在，请选择 Codex / ChatGPT.app。")?;
    let bundle = path
        .ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .ok_or("请选择 Codex 桌面 .app 应用，不要选择命令行程序。")?
        .to_path_buf();
    if plist(&bundle, "CFBundleIdentifier")? != "com.openai.codex" {
        return Err("所选应用不是 Codex 桌面客户端（com.openai.codex）。".into());
    }
    let name = plist(&bundle, "CFBundleExecutable")?;
    if name.is_empty() || name.contains('/') || name == "." || name == ".." {
        return Err("应用可执行文件名称无效。".into());
    }
    let executable = bundle.join("Contents/MacOS").join(name);
    if !executable.is_file()
        || fs::metadata(&executable)
            .map_err(|e| e.to_string())?
            .permissions()
            .mode()
            & 0o111
            == 0
    {
        return Err("应用可执行文件缺失或不可执行。".into());
    }
    Ok((bundle, executable))
}
fn discover() -> String {
    let mut roots = vec![PathBuf::from("/Applications")];
    if let Ok(h) = home() {
        roots.push(h.join("Applications"));
    }
    for root in roots {
        for name in ["Codex.app", "ChatGPT.app"] {
            if let Ok((bundle, _)) = validate(&root.join(name).to_string_lossy()) {
                return bundle.to_string_lossy().into();
            }
        }
    }
    if let Ok(output) = Command::new("/usr/bin/mdfind")
        .arg("kMDItemCFBundleIdentifier == 'com.openai.codex'")
        .output()
    {
        for path in String::from_utf8_lossy(&output.stdout).lines() {
            if let Ok((bundle, _)) = validate(path) {
                return bundle.to_string_lossy().into();
            }
        }
    }
    String::new()
}
pub(crate) fn account_usage(payload: Value) -> Result<Value, String> {
    let selected = payload["path"].as_str().unwrap_or_default();
    let path = if selected.is_empty() { discover() } else { selected.to_owned() };
    let desktop = validate(&path).ok().map(|(bundle, _)| bundle);
    crate::account_usage::query(desktop.as_deref())
}
fn is_running(executable: &Path) -> Result<bool, String> {
    let output = Command::new("/bin/ps")
        .args(["-axo", "comm="])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("无法检查正在运行的客户端，请检查进程访问权限。".into());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|line| Path::new(line.trim()) == executable))
}
fn launch_command(executable: &Path, s: &Settings) -> Result<Command, String> {
    let mut cmd = Command::new(executable);
    cmd.env("TZ", tz(s)?)
        .env_remove("ELECTRON_RUN_AS_NODE")
        .current_dir(executable.parent().ok_or("应用目录无效")?)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    Ok(cmd)
}
fn skin_bundle() -> Result<PathBuf, String> {
    for root in [PathBuf::from("/Applications"), home()?.join("Applications")] {
        let app = root.join("Codex Dream Skin.app");
        if app
            .join("Contents/Resources/engine/scripts/start-dream-skin-macos.sh")
            .is_file()
        {
            return Ok(app);
        }
    }
    Err("未找到完整的 Codex Dream Skin.app，请先安装 macOS 版。".into())
}
fn launch_skin_app() -> Result<Value, String> {
    let app = skin_bundle()?;
    Command::new("/usr/bin/open").arg(&app).spawn().map_err(|e| format!("无法启动 Dream Skin：{e}"))?;
    Ok(json!({"launched":true,"message":"Dream Skin（macOS）已启动。"}))
}
fn installed_skin_script(user_home: &Path) -> Result<PathBuf, String> {
    let root = user_home.join(".codex/codex-dream-skin-studio");
    let entry = "scripts/start-dream-skin-macos.sh";
    // DreamSkin resolves assets relative to the script. Its bundled and
    // installed engines can have the same version but different selectors.
    for file in [
        entry,
        "scripts/common-macos.sh",
        "scripts/injector.mjs",
        "assets/renderer-inject.js",
        "assets/selectors.json",
        "assets/dream-skin.css",
    ] {
        let path = root.join(file);
        if !fs::symlink_metadata(&path).is_ok_and(|m| m.is_file()) {
            return Err(format!("DreamSkin 用户引擎文件缺失或无效：{}。请打开 DreamSkin 管理器完成安装或修复，再重新应用皮肤。", path.display()));
        }
    }
    Ok(root.join(entry))
}
fn skin_port() -> Result<u16, String> {
    let state = home()?.join("Library/Application Support/CodexDreamSkinStudio/state.json");
    if !state.exists() { return Ok(9341); }
    let value: Value = serde_json::from_slice(&fs::read(state).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let port = value.get("port").and_then(Value::as_u64).unwrap_or(9341);
    if !(1024..=65535).contains(&port) { return Err("Dream Skin 端口无效。".into()); }
    Ok(port as u16)
}
fn apply_skin(
    dir: &Path, bundle: &Path, script: &Path, port: u16, launched_at: Option<Instant>,
) -> Result<Value, String> {
    let started = launched_at.unwrap_or_else(Instant::now);
    let ready = match launched_at {
        Some(_) => wait_endpoint(port, Duration::from_secs(45)),
        None if endpoint_ready(port) => Ok(()),
        None => Err("当前客户端没有可用的皮肤调试接口。请保存工作并完全退出 Codex / ChatGPT，勾选 Dream Skin 兼容启动后点击“保存并启动”。只打开 Dream Skin 管理器不会给已运行的客户端增加调试接口。".into()),
    };
    let endpoint_elapsed = started.elapsed();
    let endpoint_ready = ready.is_ok();
    let mut cmd = Command::new("/bin/bash");
    cmd.arg(script)
        .args(["--port", &port.to_string()])
        .env("CODEX_APP_BUNDLE", bundle)
        // The script compares ps start-time strings with persisted injector
        // identity. A per-client TZ must not change those comparisons.
        .env_remove("TZ")
        .env_remove("ELECTRON_RUN_AS_NODE");
    let result = ready.and_then(|()| run_logged(cmd, &dir.join("dream-skin.log"), Duration::from_secs(150)));
    let total = started.elapsed();
    let outcome = match (endpoint_ready, result.is_ok()) {
        (false, _) => "endpoint_unavailable",
        (true, false) => "unconfirmed",
        (true, true) => "verified",
    };
    let mode = if launched_at.is_some() { "launch" } else { "reapply" };
    let finished = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let script_ms = if endpoint_ready { (total - endpoint_elapsed).as_millis() } else { 0 };
    let record = format!(
        "unix_s={finished} mode={mode} status={outcome} endpoint_ms={} script_ms={script_ms} total_ms={} script={script:?}\n",
        endpoint_elapsed.as_millis(), total.as_millis(),
    );
    // Diagnostics must not replace script stderr or change its success status.
    let _ = fs::create_dir_all(dir).and_then(|()| {
        fs::OpenOptions::new().create(true).append(true)
            .open(dir.join("dream-skin-timing.log"))?.write_all(record.as_bytes())
    });
    result?;
    Ok(json!({"launched":true,"message":"Dream Skin 已完成皮肤应用和验证。当前客户端的时区保持不变。"}))
}
fn reapply_skin(dir: &Path, s: Settings) -> Result<Value, String> {
    let selected = if s.executable.trim().is_empty() { discover() } else { s.executable };
    let (bundle, executable) = validate(&selected)?;
    if !is_running(&executable)? {
        return Err("客户端尚未运行。请勾选 Dream Skin 兼容启动，再点击“保存并启动”。".into());
    }
    let script = installed_skin_script(&home()?)?;
    apply_skin(dir, &bundle, &script, skin_port()?, None)
}
fn launch(dir: &Path, s: Settings) -> Result<Value, String> {
    tz(&s)?;
    let selected = if s.executable.trim().is_empty() { discover() } else { s.executable.clone() };
    let (bundle, executable) = validate(&selected)?;
    if is_running(&executable)? {
        return Err("客户端正在运行，请保存工作并完全退出 Codex / ChatGPT 后重试。".into());
    }
    let mut cmd = launch_command(&executable, &s)?;
    let skin = if s.dream_skin_compatible {
        let script = installed_skin_script(&home()?)?;
        let port = skin_port()?;
        let listener = std::net::TcpListener::bind(("127.0.0.1", port)).map_err(|_| "Dream Skin 调试端口已占用，请先退出相关客户端。")?;
        drop(listener);
        cmd.arg("--remote-debugging-address=127.0.0.1").arg(format!("--remote-debugging-port={port}"));
        Some((script, port))
    } else { None };
    save(dir, &s)?;
    let started = Instant::now();
    let mut child = cmd.spawn().map_err(|e| format!("无法启动客户端：{e}"))?;
    std::thread::sleep(Duration::from_millis(800));
    if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
        return Err(format!("客户端启动后提前退出：{status}。请检查是否已有实例运行。"));
    }
    std::thread::spawn(move || { let _ = child.wait(); });
    if let Some((script, port)) = skin {
        apply_skin(dir, &bundle, &script, port, Some(started))?;
    }
    Ok(json!({"launched":true,"message":if s.dream_skin_compatible { "Codex 已按所选时区启动，Dream Skin 已验证应用。" } else { "Codex 已按所选时区启动（Dream Skin 兼容启动未开启）。" }}))
}
fn create_shortcut(executable: &Path, desktop: &Path) -> Result<PathBuf, String> {
    let bundle = executable
        .ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .ok_or("开发模式不支持创建快捷方式，请使用构建后的 .app。")?;
    let link = desktop.join("Codex 时区启动器.app");
    if let Ok(target) = fs::read_link(&link) {
        if target == bundle {
            return Ok(link);
        }
    }
    symlink(bundle, &link).map_err(|e| format!("无法创建桌面入口（不会覆盖已有文件）：{e}"))?;
    Ok(link)
}
pub fn execute(command: &str, payload: Value) -> Result<Value, String> {
    let dir = data_directory()?;
    match command {
        "network_info" => network_info(),
        "bootstrap" => Ok(
            json!({"settings":load(&dir)?,"zones":zones(),"localZone":"UTC","detected":discover(),"platform":"macos","settingsPath":dir.join("settings.json")}),
        ),
        "discover" => Ok(json!({"path":discover()})),
        "validate" => Ok(json!({"path":validate(payload["path"].as_str().unwrap_or_default())?.0})),
        "save" | "launch" | "reapply_dream_skin" => {
            let s = if let Some(value) = payload.get("settings").filter(|v| !v.is_null()) {
                serde_json::from_value(value.clone()).map_err(|e| e.to_string())?
            } else {
                load(&dir)?
            };
            match command {
                "launch" => launch(&dir, s),
                "reapply_dream_skin" => reapply_skin(&dir, s),
                _ => {
                    save(&dir, &s)?;
                    Ok(json!({"path":dir.join("settings.json"),"message":format!("设置已保存到 {}",dir.join("settings.json").display())}))
                }
            }
        }
        "create_shortcut" => Ok(
            json!({"created":true,"path":create_shortcut(&std::env::current_exe().map_err(|e| e.to_string())?, &home()?.join("Desktop"))?}),
        ),
        "launch_dream_skin" | "launch_dream_skin_app" => launch_skin_app(),
        _ => Err(format!("未知命令：{command}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temp(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("codex-tz-{}-{name}", std::process::id()));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn resolves_only_the_installed_dreamskin_engine() {
        let home = tempfile::tempdir().unwrap();
        let entry = "scripts/start-dream-skin-macos.sh";
        let bundled = home.path().join("Applications/Codex Dream Skin.app/Contents/Resources/engine");
        fs::create_dir_all(bundled.join("scripts")).unwrap();
        fs::write(bundled.join(entry), "").unwrap();
        assert!(installed_skin_script(home.path()).unwrap_err().contains("管理器"));

        let installed = home.path().join(".codex/codex-dream-skin-studio");
        for file in [entry, "scripts/common-macos.sh", "scripts/injector.mjs",
                     "assets/renderer-inject.js", "assets/selectors.json", "assets/dream-skin.css"] {
            let path = installed.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        assert_eq!(installed_skin_script(home.path()).unwrap(), installed.join(entry));
        fs::remove_file(installed.join("assets/selectors.json")).unwrap();
        assert!(installed_skin_script(home.path()).unwrap_err().contains("selectors.json"));
    }
    #[test]
    fn timezone_rules() {
        let mut s = Settings::default();
        s.mode = "offset".into();
        for offset in -12..=14 {
            s.offset = offset;
            let out = Command::new("/bin/date")
                .args(["-r", "0", "+%z"])
                .env("TZ", tz(&s).unwrap())
                .output()
                .unwrap();
            assert_eq!(
                String::from_utf8_lossy(&out.stdout).trim(),
                format!(
                    "{}{hour:02}00",
                    if offset < 0 { "-" } else { "+" },
                    hour = offset.abs()
                )
            );
        }
        s.offset = 15;
        assert!(tz(&s).is_err());
        s.mode = "zone".into();
        s.zone_id = "../../etc/passwd".into();
        assert!(tz(&s).is_err());
        for (zone, winter, summer) in [
            ("America/Los_Angeles", "-0800", "-0700"),
            ("Asia/Kathmandu", "+0545", "+0545"),
            ("Australia/Adelaide", "+1030", "+0930"),
        ] {
            for (epoch, expected) in [("1767225600", winter), ("1782864000", summer)] {
                let out = Command::new("/bin/date")
                    .args(["-r", epoch, "+%z"])
                    .env("TZ", zone)
                    .output()
                    .unwrap();
                assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), expected);
            }
        }
    }
    #[test]
    fn persistence() {
        let dir = temp("settings");
        assert_eq!(load(&dir).unwrap().zone_id, "Asia/Shanghai");
        let mut s = Settings::default();
        s.zone_id = "Asia/Kathmandu".into();
        s.executable = "/Applications/带 空格.app".into();
        save(&dir, &s).unwrap();
        assert_eq!(load(&dir).unwrap().executable, s.executable);
        s.offset = -5;
        s.mode = "offset".into();
        save(&dir, &s).unwrap();
        assert_eq!(load(&dir).unwrap().offset, -5);
        s.offset = 99;
        assert!(save(&dir, &s).is_err());
        assert_eq!(load(&dir).unwrap().offset, -5);
        fs::write(dir.join("settings.json"), "invalid").unwrap();
        assert!(load(&dir).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn bundles_and_shortcuts() {
        let dir = temp("bundle");
        let bundle = dir.join("测试 Client.app");
        let bin = bundle.join("Contents/MacOS");
        fs::create_dir_all(&bin).unwrap();
        let exe = bin.join("ChatGPT");
        fs::write(&exe, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
        let info = bundle.join("Contents/Info.plist");
        fs::write(&info, "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>com.openai.codex</string><key>CFBundleExecutable</key><string>ChatGPT</string></dict></plist>").unwrap();
        assert_eq!(
            validate(bundle.to_str().unwrap()).unwrap().1,
            fs::canonicalize(&exe).unwrap()
        );
        assert!(validate(exe.to_str().unwrap()).is_ok());
        let desktop = dir.join("Desktop");
        fs::create_dir(&desktop).unwrap();
        let link = create_shortcut(&exe, &desktop).unwrap();
        assert_eq!(fs::read_link(&link).unwrap(), bundle);
        assert!(create_shortcut(&exe, &desktop).is_ok());
        fs::remove_file(&link).unwrap();
        fs::write(&link, "existing").unwrap();
        assert!(create_shortcut(&exe, &desktop).is_err());
        assert_eq!(fs::read_to_string(&link).unwrap(), "existing");
        fs::write(&info, "<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>CFBundleIdentifier</key><string>other.app</string></dict></plist>").unwrap();
        assert!(validate(bundle.to_str().unwrap()).is_err());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn child_environment_isolation() {
        let before = std::env::var_os("TZ");
        let mut s = Settings::default();
        s.zone_id = "America/New_York".into();
        let out = launch_command(Path::new("/bin/sh"), &s)
            .unwrap()
            .args([
                "-c",
                "printf '%s|%s' \"$TZ\" \"${ELECTRON_RUN_AS_NODE-unset}\"",
            ])
            .stdout(Stdio::piped())
            .output()
            .unwrap();
        assert!(out.status.success());
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            "America/New_York|unset"
        );
        assert_eq!(std::env::var_os("TZ"), before);
    }
    #[test]
    fn installed_client_and_running_guard() {
        let path = discover();
        if path.is_empty() {
            return;
        }
        let (_, exe) = validate(&path).unwrap();
        if is_running(&exe).unwrap() {
            let dir = temp("guard");
            let mut s = Settings::default();
            s.executable = path;
            assert!(launch(&dir, s).unwrap_err().contains("正在运行"));
            assert!(!dir.join("settings.json").exists());
            fs::remove_dir_all(dir).unwrap();
        }
    }
}
