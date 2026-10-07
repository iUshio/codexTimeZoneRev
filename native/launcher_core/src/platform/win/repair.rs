//! Explicit, version-bound repair. Never runs from ordinary launch implicitly.
use super::{process, target, launch_log::LaunchLog};
use serde_json::{json, Value};
use std::{fs, os::windows::process::CommandExt, path::Path, process::{Command, Stdio}};

pub(super) fn supported(target: &target::LaunchTarget, observed: &process::ObservedIdentity) -> bool {
    target.package.as_ref().is_some_and(|p|
        p.full_name.starts_with("OpenAI.Codex_") &&
        p.app_user_model_id == "OpenAI.Codex_2p2nqsd0c76g0!App") &&
        observed.package_error == 15700 && observed.app_id_error == 15703
}

pub(super) fn check(target: &target::LaunchTarget, timezone: &str, log: &mut LaunchLog) -> Result<Value, String> {
    let mut child = process::Child::suspended(&target.executable, &[], timezone)?;
    let observed = child.identity();
    // A failed cleanup must never offer repair or resume execution.
    child.abort()?;
    log.record("preflight", json!({"target":target,"identity":observed,"pid":child.id,"cleaned":true}))?;
    match process::require_identity(target.package.as_ref(), &observed) {
        Ok(()) => Ok(json!({"healthy":true,"target":target})),
        Err(_) if supported(target, &observed) => Ok(required(target, &log.path)),
        Err(error) => Err(error),
    }
}

pub(super) fn required(target: &target::LaunchTarget, log_path: &Path) -> Value {
    json!({"launched":false,"repairRequired":true,"target":target,"logPath":log_path,
        "message":"检测到当前 Codex 的包身份异常，已停止启动。可点击“一键修复并重试”，确认 Windows 管理员提示后继续。"})
}

pub(super) fn run(dir: &Path, selected: &str, timezone: &str, expected: &str) -> Result<Value, String> {
    let mut log = LaunchLog::new(dir)?;
    let target = target::for_launch(selected)?;
    let package = target.package.as_ref().ok_or("当前客户端不是支持修复的 Store 版本。")?;
    if package.full_name != expected { return Err("Codex 已更新，修复请求已失效。请重新点击“保存并启动”检查新版本。".into()); }
    let before = check(&target, timezone, &mut log)?;
    if before["healthy"] == true { return Ok(json!({"repaired":false,"healthy":true,"message":"包身份已正常，无需修改权限。"})); }
    if before["repairRequired"] != true { return Err("当前问题不支持权限修复。".into()); }
    let root = dir.join("repairs");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let run_dir = tempfile::Builder::new().prefix("repair-").tempdir_in(root).map_err(|e| e.to_string())?.keep();
    // Embedded scripts travel with the DLL; no dependency on local diagnostics.
    for (name, body) in [("repair_entry.ps1", include_str!("repair_entry.ps1")), ("repair_package_acl.ps1", include_str!("repair_package_acl.ps1"))] {
        fs::write(run_dir.join(name), format!("\u{feff}{}", body.trim_start_matches('\u{feff}'))).map_err(|e| e.to_string())?;
    }
    let system = std::env::var_os("SystemRoot").ok_or("无法确定 Windows 系统目录。")?;
    let ps_home = Path::new(&system).join(r"System32\WindowsPowerShell\v1.0");
    let output = fs::File::create(run_dir.join("console.log")).map_err(|e| e.to_string())?;
    let status = Command::new(ps_home.join("powershell.exe"))
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(run_dir.join("repair_entry.ps1")).arg("-PackageFullName").arg(expected)
        .env("PSModulePath", ps_home.join("Modules")).env_remove("ELECTRON_RUN_AS_NODE")
        .current_dir(&run_dir).creation_flags(0x08000000).stdin(Stdio::null())
        .stderr(output.try_clone().map_err(|e| e.to_string())?).stdout(output)
        .status().map_err(|e| format!("无法运行修复：{e}\n日志：{}", run_dir.display()))?;
    let result: Value = fs::read(run_dir.join("result.json")).ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or_else(|| format!("修复未返回有效结果。日志：{}", run_dir.display()))?;
    log.record("repair_result", json!({"result":result,"directory":run_dir}))?;
    if result["status"] == "cancelled" {
        return Ok(json!({"cancelled":true,"message":result["message"],"logPath":run_dir}));
    }
    if !status.success() || result["status"] != "success" {
        return Err(format!("{}\n修复日志和权限备份：{}", result["message"].as_str().unwrap_or("修复失败。"), run_dir.display()));
    }
    let current = target::for_launch(selected)?;
    if current.package.as_ref() != Some(package) { return Err("修复期间 Codex 版本发生变化，请重新检查后启动。".into()); }
    if check(&current, timezone, &mut log)?["healthy"] != true {
        return Err(format!("修复后包身份仍异常，已停止重试。日志：{}", run_dir.display()));
    }
    Ok(json!({"repaired":true,"healthy":true,"logPath":run_dir,"message":"修复和身份检查通过，正在重试启动。"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_identity_returns_a_repair_offer_without_running_the_child() {
        let temp = tempfile::tempdir().unwrap();
        let target = target::LaunchTarget {
            executable: std::env::current_exe().unwrap(),
            package: Some(target::PackageIdentity {
                full_name: "OpenAI.Codex_1.2.3.4_x64__2p2nqsd0c76g0".into(),
                app_user_model_id: "OpenAI.Codex_2p2nqsd0c76g0!App".into(),
            }),
        };
        let mut log = LaunchLog::new(temp.path()).unwrap();
        let result = check(&target, "Etc/UTC", &mut log).unwrap();
        assert_eq!(result["repairRequired"], true);
        assert_eq!(result["launched"], false);
        assert!(!temp.path().join("settings.json").exists());
        assert!(!temp.path().join("repairs").exists());
        let text = fs::read_to_string(log.path).unwrap();
        assert!(text.contains("\"cleaned\":true"));
    }
    #[test]
    fn only_exact_missing_identity_can_offer_repair() {
        let mut target = target::LaunchTarget { executable: "C:\\test.exe".into(), package: Some(target::PackageIdentity {full_name:"OpenAI.Codex_1.2.3.4_x64__2p2nqsd0c76g0".into(),app_user_model_id:"OpenAI.Codex_2p2nqsd0c76g0!App".into()})};
        let mut observed = process::ObservedIdentity {package_error:15700,app_id_error:15703,package_full_name:None,app_user_model_id:None};
        assert!(supported(&target, &observed));
        observed.app_id_error = 5;
        assert!(!supported(&target, &observed));
        observed.app_id_error = 15703;
        target.package.as_mut().unwrap().app_user_model_id = "Other!App".into();
        assert!(!supported(&target, &observed));
        target.package = None;
        assert!(!supported(&target, &observed));
    }
}
