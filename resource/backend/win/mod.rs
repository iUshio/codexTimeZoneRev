use crate::common::{endpoint_ready, load, run_logged, save, settings, tz, wait_endpoint, zones, network_info, Settings};
use quick_xml::events::Event;
use serde_json::{json, Value};
use std::{fs, mem::size_of, os::windows::{ffi::OsStrExt, process::CommandExt}, path::{Path, PathBuf}, process::{Command, Stdio}, time::Duration};
use ::windows::{
    core::{w, GUID, HSTRING, Interface, PCWSTR, PWSTR},
    Management::Deployment::PackageManager,
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::{
            Com::{CoCreateInstance, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER},
            Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS},
            RemoteDesktop::ProcessIdToSessionId,
            Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION},
            WinRT::{RoInitialize, RoUninitialize, RO_INIT_SINGLETHREADED},
        },
        UI::Shell::{FOLDERID_Desktop, FOLDERID_LocalAppData, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath, ShellLink},
    },
};

struct Runtime;
impl Runtime {
    fn new() -> Result<Self, String> {
        unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }.map_err(|e| e.to_string())?;
        Ok(Self)
    }
}
impl Drop for Runtime { fn drop(&mut self) { unsafe { RoUninitialize(); } } }
struct Handle(HANDLE);
impl Drop for Handle { fn drop(&mut self) { unsafe { let _ = CloseHandle(self.0); } } }
fn known_folder(id: &GUID) -> Result<PathBuf, String> {
    unsafe {
        let pointer = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).map_err(|e| e.to_string())?;
        let value = pointer.to_string().map_err(|e| e.to_string());
        CoTaskMemFree(Some(pointer.0.cast()));
        value.map(PathBuf::from)
    }
}
fn wide(path: &Path) -> Vec<u16> { path.as_os_str().encode_wide().chain(Some(0)).collect() }
fn data_directory() -> Result<PathBuf, String> {
    Ok(std::env::current_exe().map_err(|e| e.to_string())?.parent().ok_or("无法确定程序目录。")?.join("data"))
}
fn load_migrated(dir: &Path) -> Result<Settings, String> {
    if dir.join("settings.json").exists() { return load(dir); }
    let legacy = known_folder(&FOLDERID_LocalAppData)?.join("ChatGPTTimeZoneLauncher");
    if legacy.join("settings.json").exists() {
        if let Ok(s) = load(&legacy) { save(dir, &s)?; return Ok(s); }
    }
    Ok(Settings::default())
}
fn validate(path: &str) -> Result<PathBuf, String> {
    let path = fs::canonicalize(path.trim().trim_matches('"')).map_err(|_| "客户端路径不存在，请选择 Codex 桌面程序。")?;
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
    if !path.is_file() || !["codex.exe", "chatgpt.exe"].contains(&name.as_str()) || !path.parent().ok_or("客户端目录无效")?.join("icudtl.dat").is_file() {
        return Err("请选择 Codex 桌面安装目录中的 Codex.exe / ChatGPT.exe，不要选择命令行程序。".into());
    }
    Ok(path)
}
fn manifest_executable(root: &Path) -> Option<PathBuf> {
    let root = fs::canonicalize(root).ok()?;
    let text = fs::read_to_string(root.join("AppxManifest.xml")).ok()?;
    let mut reader = quick_xml::Reader::from_str(&text);
    reader.config_mut().trim_text(true);
    let mut executable_name = None;
    loop {
        match reader.read_event().ok()? {
            Event::Start(element) if element.local_name().as_ref() == "Application" => {
                let mut is_app = false;
                let mut executable = None;
                for attribute in element.attributes().flatten() {
                    match attribute.key.local_name().as_ref() {
                        "Id" => is_app = attribute.value.as_ref() == "App",
                        "Executable" => executable = Some(attribute.value.into_owned()),
                        _ => {}
                    }
                }
                if is_app {
                    executable_name = executable;
                    break;
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let executable = fs::canonicalize(root.join(executable_name?.replace('/', "\\"))).ok()?;
    if !executable.starts_with(&root) { return None; }
    validate(&executable.to_string_lossy()).ok()
}
fn discover() -> String {
    let mut candidates = Vec::new();
    if let Ok(manager) = PackageManager::new() {
        if let Ok(packages) = manager.FindPackagesByUserSecurityId(&HSTRING::new()) {
            for package in packages {
                let found = || -> ::windows::core::Result<_> {
                    let id = package.Id()?;
                    let name = id.Name()?.to_string_lossy();
                    let v = id.Version()?;
                    let root = package.InstalledLocation()?.Path()?.to_string_lossy();
                    Ok((name, (v.Major, v.Minor, v.Build, v.Revision), PathBuf::from(root)))
                };
                if let Ok((name, version, root)) = found() {
                    if name.eq_ignore_ascii_case("OpenAI.Codex") { candidates.push((version, root)); }
                }
            }
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, root) in candidates {
        if let Some(path) = manifest_executable(&root) { return path.to_string_lossy().into_owned(); }
    }
    if let Ok(local) = known_folder(&FOLDERID_LocalAppData) {
        for relative in [r"Programs\Codex\Codex.exe", r"Codex\Codex.exe", r"Programs\OpenAI\Codex\Codex.exe"] {
            if let Ok(path) = validate(&local.join(relative).to_string_lossy()) { return path.to_string_lossy().into_owned(); }
        }
    }
    String::new()
}
fn same_path(a: &Path, b: &Path) -> bool {
    let normalize = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().trim_start_matches(r"\\?\").to_lowercase();
    normalize(a) == normalize(b)
}
fn is_running(executable: &Path) -> Result<bool, String> {
    unsafe {
        let mut session = 0;
        ProcessIdToSessionId(std::process::id(), &mut session).map_err(|e| e.to_string())?;
        let snapshot = Handle(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).map_err(|e| e.to_string())?);
        let mut entry = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        if Process32FirstW(snapshot.0, &mut entry).is_err() { return Ok(false); }
        loop {
            let end = entry.szExeFile.iter().position(|c| *c == 0).unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_lowercase();
            if ["codex.exe", "chatgpt.exe"].contains(&name.as_str()) {
                let mut candidate_session = 0;
                if ProcessIdToSessionId(entry.th32ProcessID, &mut candidate_session).is_ok() && session == candidate_session {
                    let process = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, entry.th32ProcessID) {
                        Ok(p) => Handle(p), Err(_) => return Ok(true),
                    };
                    let mut buffer = vec![0u16; 32768]; let mut length = buffer.len() as u32;
                    if QueryFullProcessImageNameW(process.0, PROCESS_NAME_WIN32, PWSTR(buffer.as_mut_ptr()), &mut length).is_err() { return Ok(true); }
                    if same_path(executable, &PathBuf::from(String::from_utf16_lossy(&buffer[..length as usize]))) { return Ok(true); }
                }
            }
            if Process32NextW(snapshot.0, &mut entry).is_err() { break; }
        }
    }
    Ok(false)
}
fn create_shortcut() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let desktop = known_folder(&FOLDERID_Desktop)?;
    let path = desktop.join("Codex 时区启动器.lnk");
    // Stage the COM-created shortcut before replacing the destination atomically.
    let staging = tempfile::tempdir_in(&desktop).map_err(|e| e.to_string())?;
    let staged = staging.path().join("launcher.lnk");
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?;
        link.SetPath(PCWSTR(wide(&executable).as_ptr())).map_err(|e| e.to_string())?;
        link.SetWorkingDirectory(PCWSTR(wide(executable.parent().ok_or("程序目录无效")?).as_ptr())).map_err(|e| e.to_string())?;
        link.SetDescription(w!("选择时区并启动 Codex")).map_err(|e| e.to_string())?;
        link.SetIconLocation(PCWSTR(wide(&executable).as_ptr()), 0).map_err(|e| e.to_string())?;
        let persist: IPersistFile = link.cast().map_err(|e| e.to_string())?;
        persist.Save(PCWSTR(wide(&staged).as_ptr()), true).map_err(|e| e.to_string())?;
    }
    let mut file = tempfile::NamedTempFile::new_in(&desktop).map_err(|e| e.to_string())?;
    use std::io::Write;
    file.write_all(&fs::read(staged).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    file.persist(&path).map_err(|e| e.to_string())?;
    Ok(path)
}
fn skin_root() -> Result<PathBuf, String> { Ok(known_folder(&FOLDERID_LocalAppData)?.join("CodexDreamSkin")) }
fn skin_state(root: &Path) -> Result<Value, String> {
    let path = root.join("state.json");
    if !path.exists() { return Ok(json!({})); }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(bytes.strip_prefix(&[0xef,0xbb,0xbf]).unwrap_or(&bytes)).map_err(|e| e.to_string())
}
fn skin_port(state: &Value) -> Result<u16, String> {
    let port = state.get("port").and_then(Value::as_u64).unwrap_or(9335);
    if !(1024..=65535).contains(&port) { return Err("Dream Skin 端口无效。".into()); }
    Ok(port as u16)
}
fn skin_script(root: &Path, state: &Value) -> Result<PathBuf, String> {
    let mut candidates = vec![root.join("engine/scripts"), root.join("scripts")];
    if let Some(injector) = state["injectorPath"].as_str().map(PathBuf::from).filter(|p| p.is_absolute()) {
        if let Some(parent) = injector.parent() { candidates.push(parent.to_path_buf()); }
    }
    for directory in candidates {
        if ["tray-dream-skin.ps1", "common-windows.ps1", "theme-windows.ps1", "localization-windows.ps1", "start-dream-skin.ps1"].iter().all(|name| directory.join(name).is_file()) {
            return Ok(directory.join("start-dream-skin.ps1"));
        }
    }
    Err("未找到完整的 Windows Dream Skin 安装。".into())
}
fn skin_tray_script(root: &Path, state: &Value) -> Result<PathBuf, String> {
    let mut candidates = vec![root.join("engine/scripts"), root.join("scripts")];
    if let Some(injector) = state["injectorPath"].as_str().map(PathBuf::from).filter(|p| p.is_absolute()) {
        if let Some(parent) = injector.parent() { candidates.push(parent.to_path_buf()); }
    }
    for directory in candidates {
        let tray = directory.join("tray-dream-skin.ps1");
        let complete = ["common-windows.ps1", "theme-windows.ps1", "localization-windows.ps1", "start-dream-skin.ps1"]
            .iter().all(|name| directory.join(name).is_file());
        if tray.is_file() && complete { return Ok(tray); }
    }
    Err("未找到完整的 Windows Dream Skin 安装。".into())
}
fn launch_dream_skin(root: &Path, state: &Value) -> Result<Value, String> {
    let tray = skin_tray_script(root, state)?;
    let system = std::env::var_os("SystemRoot").ok_or("无法确定 Windows 系统目录")?;
    let mut command = Command::new(PathBuf::from(system).join(r"System32\WindowsPowerShell\v1.0\powershell.exe"));
    let working_dir = tray.parent().and_then(Path::parent).ok_or("Dream Skin 脚本目录无效")?;
    command.args(["-NoProfile", "-STA", "-WindowStyle", "Hidden", "-ExecutionPolicy", "RemoteSigned", "-File"])
        .arg(&tray).current_dir(working_dir).creation_flags(0x08000000)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .env_remove("TZ").env_remove("ELECTRON_RUN_AS_NODE");
    command.spawn().map_err(|e| format!("无法启动 Dream Skin：{e}"))?;
    Ok(json!({"launched":true,"path":tray,"message":"Dream Skin 已启动。"}))
}
fn apply_skin(dir: &Path, script: &Path, port: u16) -> Result<Value, String> {
    if !endpoint_ready(port) { return Err("已保存兼容启动设置，但当前客户端没有皮肤调试接口。请保存工作并完全退出客户端，再点击“保存并启动 Codex”。".into()); }
    let system = std::env::var_os("SystemRoot").ok_or("无法确定 Windows 系统目录")?;
    let mut command = Command::new(PathBuf::from(system).join(r"System32\WindowsPowerShell\v1.0\powershell.exe"));
    command.args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-ExecutionPolicy", "RemoteSigned", "-File"])
        .arg(script).args(["-OperationLockTimeoutMilliseconds", "300000"])
        .current_dir(script.parent().ok_or("皮肤脚本目录无效")?)
        .creation_flags(0x08000000).env_remove("TZ").env_remove("ELECTRON_RUN_AS_NODE");
    run_logged(command, &dir.join("dream-skin.log"), Duration::from_secs(330))?;
    Ok(json!({"launched":true,"message":"Dream Skin 已完成皮肤应用和验证。"}))
}
fn launch(dir: &Path, s: Settings) -> Result<Value, String> {
    tz(&s)?;
    let path = validate(&if s.executable.trim().is_empty() { discover() } else { s.executable.clone() })?;
    if is_running(&path)? { return Err("客户端正在运行，请保存工作并完全退出 Codex 后重试。".into()); }
    let mut cmd = Command::new(&path);
    cmd.env("TZ", tz(&s)?).env_remove("ELECTRON_RUN_AS_NODE").current_dir(path.parent().ok_or("客户端目录无效")?)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(0x08000000);
    let skin = if s.dream_skin_compatible {
        let root = skin_root()?; let state = skin_state(&root)?; let port = skin_port(&state)?; let script = skin_script(&root, &state)?;
        let profile = state["profilePath"].as_str().filter(|s| !s.trim().is_empty()).map(PathBuf::from).unwrap_or_else(|| root.join("cdp-profile"));
        if !profile.is_absolute() { return Err("Dream Skin 用户目录必须是绝对路径。".into()); }
        let listener = std::net::TcpListener::bind(("127.0.0.1", port)).map_err(|_| "Dream Skin 调试端口已被占用。")?; drop(listener);
        cmd.arg("--remote-debugging-address=127.0.0.1").arg(format!("--remote-debugging-port={port}")).arg(format!("--user-data-dir={}", profile.display()));
        Some((script, port))
    } else { None };
    save(dir, &s)?;
    let mut child = cmd.spawn().map_err(|e| format!("无法启动客户端：{e}"))?;
    std::thread::sleep(Duration::from_millis(800));
    if let Some(status) = child.try_wait().map_err(|e| e.to_string())? { return Err(format!("客户端启动后提前退出：{status}。")); }
    std::thread::spawn(move || { let _ = child.wait(); });
    if let Some((script, port)) = skin { wait_endpoint(port, Duration::from_secs(45))?; apply_skin(dir, &script, port)?; }
    Ok(json!({"launched":true,"message":if s.dream_skin_compatible { "Codex 已按所选时区启动，Dream Skin 已应用皮肤。" } else { "Codex 已按所选时区启动（Dream Skin 兼容启动未开启）。" }}))
}
pub fn execute(command: &str, payload: Value) -> Result<Value, String> {
    let _runtime = Runtime::new()?;
    let dir = data_directory()?;
    match command {
        "network_info" => network_info(),
        "bootstrap" => Ok(json!({"settings":load_migrated(&dir)?,"zones":zones(),"localZone":"UTC","detected":discover(),"platform":"windows","settingsPath":dir.join("settings.json")})),
        "discover" => Ok(json!({"path":discover()})),
        "validate" => Ok(json!({"path":validate(payload["path"].as_str().unwrap_or_default())?})),
        "save" => { save(&dir, &settings(&payload, &dir)?)?; Ok(json!({"path":dir.join("settings.json"),"message":format!("设置已保存到 {}", dir.join("settings.json").display())})) },
        "launch" => launch(&dir, settings(&payload, &dir)?),
        "create_shortcut" => Ok(json!({"created":true,"path":create_shortcut()?})),
        "launch_dream_skin" | "launch_dream_skin_app" => {
            let root = skin_root()?; let state = skin_state(&root)?; launch_dream_skin(&root, &state)
        },
        _ => Err(format!("未知命令：{command}")),
    }
}
