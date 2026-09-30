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

对于 Microsoft Store / MSIX 安装的客户端，启动器会在新进程执行前核对程序包与应用身份。若提示 `15700`（进程没有程序包标识符），本次新进程会被取消；请检查客户端安装状态及其 WindowsApps 目录权限。不要通过向普通用户授予整个 WindowsApps 目录执行权限来处理此错误，这可能破坏 Windows 的包身份启动机制。启动器不会自动修改这些权限。

启动阶段、目标版本、所选时区和原生错误会写入启动器旁的 `data/launch.log`，不记录完整环境变量。普通启动提示仅确认进程已启动；客户端窗口就绪及最终显示的时区仍需实际确认。

## 从源码构建

需要 Flutter 3.47.5（Dart 3.13.4）、Rust stable。Windows 还需要 Visual Studio C++ 桌面工具链；macOS 需要完整 Xcode。将 Flutter 的 `bin` 与 Cargo 加入 `PATH`，或在 Windows 传入 `-FlutterSdk`。

```powershell
# Windows：在仓库根目录运行
.\scripts\flutter-windows.ps1 -Action test
.\scripts\test-native-windows.ps1
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
