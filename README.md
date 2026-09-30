# Codex 时区启动器

**本程序由 Codex 开发。** 这是独立的开源桌面项目，并非 OpenAI 官方应用。

这是一个使用 Flutter、Forui 和 Rust 构建的 Windows/macOS 启动器。它为新启动的客户端进程设置指定时区，不修改操作系统的全局时区；已经运行的客户端不会因修改设置而自动切换时区。

## 主要功能

- 选择 IANA 地区时区（含夏令时规则）或 UTC−12 至 UTC+14 的固定偏移，预览本地与目标时间。
- 自动发现或手动选择客户端，保存配置、检查启动条件并创建桌面快捷方式。
- 查询双线路公网信息、缓存结果、复制 IP，并将可识别的时区用于设置草稿。公网查询会连接第三方服务。
- 支持 Dream Skin 兼容启动、打开管理器和“重新注入皮肤”。重新注入需要当前客户端通过兼容模式启动并开放本机调试端口；此操作不会重启客户端或改变其时区。
- 读取旧版 `settings.json`；用户配置保存在本机，发布包不包含本机设置或日志。

## 下载与运行

在 [GitHub Releases](https://github.com/iUshio/codexTimeZoneRev/releases) 下载对应平台的 ZIP。Windows 解压后运行 `codex_timezone.exe`，并保留同目录的 DLL 和 `data` 资源。macOS 解压后运行应用；当前 macOS 包仅使用临时签名，尚未完成 Developer ID 签名与公证，系统可能要求用户手动确认打开。

### Windows 启动排错

对于 Microsoft Store / MSIX 安装的客户端，每次启动都会重新识别当前注册版本；设置中保存的旧 Store 版本路径会跟随更新。普通手动安装路径保持原选择。新进程在执行前核对程序包与应用身份。

若检测到受支持的 `15700 / 15703` 身份缺失，点击“一键修复并重试”，再确认 Windows 管理员提示。启动器先备份权限，仅将当前 Codex 包的一条已知异常普通用户规则从读取和执行缩小为读取，保留包身份条件执行权限、所有者及其他规则。完成后重新检查身份，并按原时区和皮肤设置重试一次。取消授权、权限不符合条件、版本变化或复检失败都会停止；不会关闭现有客户端。正常启动不会请求管理员权限。

权限备份、修复结果和日志保存在 `data/repairs/repair-*/`。需要回退时，核对该次 `plan.json` 后在管理员 Windows PowerShell 中运行同目录的 `repair_package_acl.ps1 -Mode Rollback -PlanPath <plan.json绝对路径>`；脚本仅接受原版本和符合备份的权限状态。回退可能恢复原启动故障。请勿重置整个 WindowsApps 的权限。

此流程用于在更新后再次出现已知权限问题时恢复启动，不保证未来客户端版本或未知权限变化均可自动修复。修复脚本内置在 DLL 中，完整发布包不依赖本机 diagnostics 文件夹。

启动阶段、目标版本、所选时区和原生错误会写入启动器旁的 `data/launch.log`，不记录完整环境变量。普通启动提示仅确认进程已启动；客户端窗口就绪及最终显示的时区仍需实际确认。

## 从源码构建

需要 Flutter 3.47.5（Dart 3.13.4）、Rust stable。Windows 还需要 Visual Studio C++ 桌面工具链；macOS 需要完整 Xcode。将 Flutter 的 `bin` 与 Cargo 加入 `PATH`，或在 Windows 传入 `-FlutterSdk`。

```powershell
# Windows：在仓库根目录运行
.\scripts\flutter-windows.ps1 -Action test
.\scripts\test-native-windows.ps1
.\scripts\test-package-acl-repair.ps1
.\scripts\test-repair-entry.ps1
# 可选：以安装 Codex 的用户验证当前包身份、旧路径跟随更新和过期修复拒绝；不执行客户端代码。
.\scripts\test-native-windows.ps1 -InstalledPreflight
.\scripts\flutter-windows.ps1 -Action build
```

```bash
# macOS：在仓库根目录运行
bash scripts/flutter-macos.sh test
bash scripts/flutter-macos.sh build
```

构建产物位于 `environment/artifacts/`。发布版的 Windows/macOS 云端构建仅由 GitHub Release 的发布事件触发；向 `main` 推送代码不会启动此工作流。构建完成后，ZIP 会附加到该 Release。

## 项目结构

| 目录 | 内容 |
| --- | --- |
| `flutter_app/` | Flutter + Forui 界面、平台工程与测试 |
| `native/launcher_core/` | Rust 原生后端、跨平台逻辑与测试 |
| `scripts/` | 本地测试、构建与打包脚本 |
| `.github/workflows/` | 发布时运行的双平台构建工作流 |

开发模式、配置迁移和平台限制见 [运行说明](flutter_app/README.md)。历史迁移过程见 [迁移记录](FLUTTER_MIGRATION_PLAN.md)。项目按 [LICENSE](LICENSE) 授权。
