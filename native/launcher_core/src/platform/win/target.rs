//! Resolve both the executable and the registered identity used to launch it.
use super::{known_folder, same_path, validate};
use ::windows::{
    core::HSTRING, Management::Deployment::PackageManager, Win32::UI::Shell::FOLDERID_LocalAppData,
};
use quick_xml::events::Event;
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PackageIdentity {
    pub full_name: String,
    pub app_user_model_id: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LaunchTarget {
    pub executable: PathBuf,
    pub package: Option<PackageIdentity>,
}

struct RegisteredPackage {
    name: String,
    version: (u16, u16, u16, u16),
    root: PathBuf,
    full_name: String,
    family_name: String,
}

fn registered_packages() -> Result<Vec<RegisteredPackage>, String> {
    let manager = PackageManager::new().map_err(|e| format!("无法查询客户端程序包：{e}"))?;
    let packages = manager
        .FindPackagesByUserSecurityId(&HSTRING::new())
        .map_err(|e| format!("无法查询当前用户的客户端程序包：{e}"))?;
    let mut result = Vec::new();
    for package in packages {
        let read = || -> ::windows::core::Result<RegisteredPackage> {
            let id = package.Id()?;
            let v = id.Version()?;
            Ok(RegisteredPackage {
                name: id.Name()?.to_string_lossy(),
                version: (v.Major, v.Minor, v.Build, v.Revision),
                root: PathBuf::from(package.InstalledLocation()?.Path()?.to_string_lossy()),
                full_name: id.FullName()?.to_string_lossy(),
                family_name: id.FamilyName()?.to_string_lossy(),
            })
        };
        // An unrelated resource/framework package can lack an accessible location.
        if let Ok(package) = read() {
            result.push(package);
        }
    }
    Ok(result)
}

fn normalized(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .trim_end_matches(['\\', '/'])
        .to_lowercase()
}

fn contains(root: &Path, executable: &Path) -> bool {
    normalized(executable).starts_with(&(normalized(root) + "\\"))
}

fn manifest_targets(package: &RegisteredPackage) -> Result<Vec<LaunchTarget>, String> {
    let root =
        fs::canonicalize(&package.root).map_err(|e| format!("无法读取客户端程序包目录：{e}"))?;
    let text = fs::read_to_string(root.join("AppxManifest.xml"))
        .map_err(|e| format!("无法读取客户端程序包清单：{e}"))?;
    let mut reader = quick_xml::Reader::from_str(&text);
    reader.config_mut().trim_text(true);
    let mut targets = Vec::new();
    loop {
        match reader
            .read_event()
            .map_err(|e| format!("客户端程序包清单无效：{e}"))?
        {
            Event::Start(element) | Event::Empty(element)
                if element.local_name().as_ref() == "Application" =>
            {
                let mut app_id = None;
                let mut executable = None;
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|e| format!("客户端程序包属性无效：{e}"))?;
                    let value = attribute
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(|e| format!("客户端程序包属性无效：{e}"))?
                        .into_owned();
                    match attribute.key.local_name().as_ref() {
                        "Id" => app_id = Some(value),
                        "Executable" => executable = Some(value),
                        _ => {}
                    }
                }
                if let (Some(app_id), Some(relative)) = (app_id, executable) {
                    if app_id.is_empty() {
                        continue;
                    }
                    if let Ok(executable) =
                        validate(&root.join(relative.replace('/', "\\")).to_string_lossy())
                    {
                        if contains(&root, &executable) {
                            targets.push(LaunchTarget {
                                executable,
                                package: Some(PackageIdentity {
                                    full_name: package.full_name.clone(),
                                    app_user_model_id: format!("{}!{app_id}", package.family_name),
                                }),
                            });
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(targets)
}

fn looks_packaged(executable: &Path) -> bool {
    executable.ancestors().any(|p| {
        p.file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("WindowsApps"))
            || p.join("AppxManifest.xml").is_file()
    })
}

/// Manual selection is subject to the same registered identity checks as discovery.
pub(super) fn resolve(path: &str) -> Result<LaunchTarget, String> {
    let executable = validate(path)?;
    let packages = match registered_packages() {
        Ok(packages) => packages,
        Err(error) if looks_packaged(&executable) => return Err(error),
        Err(_) => Vec::new(),
    };
    for package in packages {
        let root = fs::canonicalize(&package.root).unwrap_or(package.root.clone());
        if contains(&root, &executable) {
            return manifest_targets(&package)?
                .into_iter()
                .find(|target| same_path(&target.executable, &executable))
                .ok_or_else(|| {
                    "所选程序不是已注册程序包清单中的 Codex 桌面启动入口，请重新自动检测。".into()
                });
        }
    }
    if looks_packaged(&executable) {
        return Err("所选客户端位于程序包目录，但未找到当前用户的有效包注册信息。请通过 Windows 应用设置修复客户端安装后重新自动检测。".into());
    }
    Ok(LaunchTarget {
        executable,
        package: None,
    })
}

pub(super) fn discover() -> Option<LaunchTarget> {
    let mut packages = registered_packages().unwrap_or_default();
    packages.retain(|package| package.name.eq_ignore_ascii_case("OpenAI.Codex"));
    packages.sort_by(|a, b| b.version.cmp(&a.version));
    for package in packages {
        if let Ok(mut targets) = manifest_targets(&package) {
            targets.sort_by_key(|target| {
                !target
                    .package
                    .as_ref()
                    .unwrap()
                    .app_user_model_id
                    .ends_with("!App")
            });
            if let Some(target) = targets.into_iter().next() {
                return Some(target);
            }
        }
    }
    if let Ok(local) = known_folder(&FOLDERID_LocalAppData) {
        for relative in [
            r"Programs\Codex\Codex.exe",
            r"Codex\Codex.exe",
            r"Programs\OpenAI\Codex\Codex.exe",
        ] {
            if let Ok(target) = resolve(&local.join(relative).to_string_lossy()) {
                return Some(target);
            }
        }
    }
    None
}

// Recognize only a Store Codex path. Ordinary manual installations keep their
// selected executable, including when that path becomes invalid.
fn store_codex_path(path: &str) -> bool {
    let text = path.trim().trim_matches('"').replace('/', "\\").to_lowercase();
    let parts: Vec<_> = text.split('\\').collect();
    parts.windows(2).any(|pair| {
        if pair[0] != "windowsapps" { return false; }
        let Some(name) = pair[1].strip_prefix("openai.codex_") else { return false; };
        let Some(version_arch) = name.strip_suffix("__2p2nqsd0c76g0") else { return false; };
        let Some((version, arch)) = version_arch.rsplit_once('_') else { return false; };
        let numbers: Vec<_> = version.split('.').collect();
        ["x64", "arm64"].contains(&arch) && numbers.len() == 4 &&
            numbers.iter().all(|n| !n.is_empty() && n.parse::<u16>().is_ok())
    })
}

pub(super) fn for_launch(path: &str) -> Result<LaunchTarget, String> {
    if path.trim().is_empty() || store_codex_path(path) {
        let target = discover().ok_or("未找到当前已安装的 Codex，请重新选择客户端。")?;
        if !path.trim().is_empty() && target.package.is_none() {
            return Err("原 Store 客户端已不可用，请重新选择客户端。".into());
        }
        Ok(target)
    } else {
        resolve(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_store_paths_follow_updates_but_manual_paths_do_not() {
        assert!(store_codex_path(r"\\?\C:\Program Files\WindowsApps\OpenAI.Codex_26.928.2636.0_x64__2p2nqsd0c76g0\app\ChatGPT.exe"));
        assert!(store_codex_path(r"C:\Program Files\WindowsApps\OpenAI.Codex_99.1.2.3_arm64__2p2nqsd0c76g0\app\Codex.exe"));
        for path in [r"C:\Custom\Codex.exe", r"C:\WindowsApps\OpenAI.Codex_bad_x64__2p2nqsd0c76g0\app\ChatGPT.exe", r"C:\WindowsApps\OpenAI.Codex_1.2.3.4_x64__other\app\ChatGPT.exe"] {
            assert!(!store_codex_path(path));
        }
    }

    #[test]
    fn manifest_keeps_identity_and_accepts_non_default_application_id() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("ChatGPT.exe"), b"fixture").unwrap();
        fs::write(temp.path().join("icudtl.dat"), b"").unwrap();
        fs::write(temp.path().join("AppxManifest.xml"), r#"<Package><Applications><Application Id="Desktop" Executable="ChatGPT.exe" /></Applications></Package>"#).unwrap();
        let package = RegisteredPackage {
            name: "Test".into(),
            version: (1, 0, 0, 0),
            root: temp.path().into(),
            full_name: "Test_1.0.0.0_x64__test".into(),
            family_name: "Test_test".into(),
        };
        let targets = manifest_targets(&package).unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(
            targets[0].package.as_ref().unwrap().app_user_model_id,
            "Test_test!Desktop"
        );
        assert_eq!(
            targets[0].package.as_ref().unwrap().full_name,
            package.full_name
        );
    }

    #[test]
    fn package_path_boundary_does_not_match_a_sibling() {
        assert!(contains(
            Path::new(r"C:\Apps\Codex"),
            Path::new(r"\\?\C:\Apps\Codex\app\ChatGPT.exe")
        ));
        assert!(!contains(
            Path::new(r"C:\Apps\Codex"),
            Path::new(r"C:\Apps\Codex-other\ChatGPT.exe")
        ));
    }
}
