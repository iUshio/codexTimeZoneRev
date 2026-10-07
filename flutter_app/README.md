# Flutter + Forui 桌面应用

本程序由 Codex 开发。此目录包含桌面界面、Windows/macOS runner 与 Dart 测试；Rust 后端位于 `../native/launcher_core/`。

## 环境

- Flutter 3.47.5 / Dart 3.13.4，Forui 0.27.0（由 `pubspec.lock` 固定）。
- Rust stable；Windows 需要 Visual Studio C++ 桌面工具链，macOS 需要完整 Xcode。
- 将 Flutter 的 `bin`、Cargo 加入 `PATH`。Windows 也可设置 `FLUTTER_ROOT` 或向脚本传入 `-FlutterSdk`。依赖缓存使用工具默认位置，也可通过 `PUB_CACHE` 自行指定。

## Windows

在仓库根目录运行：

```powershell
.\scripts\flutter-windows.ps1 -Action doctor
.\scripts\flutter-windows.ps1 -Action test
.\scripts\flutter-windows.ps1 -Action preview
.\scripts\flutter-windows.ps1 -Action run
.\scripts\flutter-windows.ps1 -Action build
.\scripts\test-native-windows.ps1
```

`preview` 使用固定演示数据，不保存设置或访问外部应用。`run` 使用实际配置与原生后端，启动界面时会查询公网信息。`test-native-windows.ps1` 在忽略提交的 `environment/` 目录内创建隔离测试客户端，不启动真实客户端。

便携 ZIP 为 `environment/artifacts/CodexTimeZone-Flutter-windows-x64.zip`。解压后运行 `codex_timezone.exe`，保留整个目录。直接执行 `flutter run` 前，需要自行编译 Rust 核心，并将 `CODEX_TZ_NATIVE_LIBRARY` 指向生成的 DLL。

## macOS

```bash
bash scripts/flutter-macos.sh test
bash scripts/flutter-macos.sh preview
bash scripts/flutter-macos.sh run
bash scripts/flutter-macos.sh build
```

脚本将 Rust 原生库复制进应用并对本地构建进行临时签名，ZIP 位于 `environment/artifacts/`。已在 Apple Silicon Mac 上完成构建、签名检查和 Dream Skin 重新注入验证；Developer ID 签名、公证及其他机器上的回归仍待完成。当前没有通用二进制包承诺。

## 配置与限制

- Windows 配置在程序旁的 `data/settings.json`；macOS 配置在应用支持目录。旧版配置格式由原生后端兼容读取。
- Flutter 的主题偏好和网络缓存独立保存；旧 WebView 的主题与缓存不会自动迁移。
- 地区时区清单包含 120 个常用地区，重点覆盖美国、亚洲和欧洲，并使用完整时区数据支持清单中的 IANA 标识。可在地区时区菜单中搜索中文城市名称或 IANA 标识。
- ZIP 只包含应用资源，不包含本机设置或日志。
- Dream Skin 重新注入要求客户端已经通过兼容模式启动；仅打开管理器不能为已运行的客户端补上调试接口。
- Windows SOCKS 或需要认证的代理、真实客户端与文件选择器等系统交互仍需按实际环境验证。

发布工作流见 `../.github/workflows/flutter-desktop.yml`，只在 GitHub Release 发布时执行 Windows/macOS 检查和构建，并把 ZIP 添加到 Release。外观与 RTSS 说明见 [Windows 外观记录](../WINDOWS_APPEARANCE.md)。
