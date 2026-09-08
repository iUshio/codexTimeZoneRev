# Codex 时区启动器

> **本项目由 OpenAI Codex 开发。** 核心源码、跨平台后端、MacVue 界面与构建流程均由 Codex 在需求指导下完成。

Codex 时区启动器是一款面向 Windows 和 macOS 的桌面工具。它会为新启动的 Codex 进程设置独立时区，让 Codex 按目标地区显示和解析时间，同时不修改操作系统的全局时区。

## 项目亮点

- **进程级时区隔离**：仅影响由本工具启动的 Codex，不打扰浏览器、日历和其他应用。
- **两种时区模式**：支持带夏令时规则的 IANA 时区，也支持 UTC−12:00 至 UTC+14:00 的固定偏移。
- **完整启动流程**：自动发现 Codex、手动选择路径、实时校验、保存设置、启动应用和创建快捷方式。
- **目标时间预览**：同时展示本地时间与目标时间，修改草稿后可立即确认效果。
- **公网 IP 与位置参考**：国内、国际线路独立查询，支持多服务商容错、结果缓存、手动刷新、复制 IP，以及将识别到的时区写入当前草稿。
- **Dream Skin 兼容**：可保存皮肤兼容开关，并从界面打开 Dream Skin 管理工具；“打开工具”和“应用设置”的职责清晰分离。
- **安全的初始化状态**：配置尚未加载或加载失败时，会阻止误保存和误启动，并提供明确的重试入口。

## 全新界面与交互

本次重设计基于 MacVue 组件体系，统一了桌面端的视觉、状态与反馈：

- 顶部工具栏、设置卡片、时间预览、网络信息和固定操作栏形成清晰的信息层级。
- 支持跟随系统、浅色和深色主题；弹出菜单通过 Portal 渲染时也能继承正确主题。
- 适配 1280×900、760×900 和 600×800 等窗口尺寸，窄屏下会自动重排，避免横向溢出。
- 未保存修改、校验结果、后台任务和错误信息均使用明确的状态表达。
- 补充键盘焦点、语义标签、减少动态效果和减少透明度等无障碍支持。
- 缓存时区格式化器、在窗口隐藏时暂停时钟刷新，并取消无意义轮询，降低长期运行开销。

## 使用前须知

1. 使用启动器前，请先完全退出正在运行的 Codex。
2. 选择 IANA 时区可自动遵循夏令时；固定 UTC 偏移不会自动处理夏令时变化。
3. 启动器只向子进程注入时区环境，不会更改系统设置。
4. “应用识别时区”只更新界面中的设置草稿，需要保存后才会用于后续启动。

## 支持平台

| 平台 | 状态 | 说明 |
| --- | --- | --- |
| Windows 10/11 x64 | 已完成 Release 构建验证 | 运行界面需要 Microsoft Edge WebView2 Runtime |
| macOS | 已提供原生实现与构建脚本 | 发布前建议在目标 macOS 版本完成签名、公证和实机回归 |

## 获取与运行

正式发布后，建议从 GitHub Releases 下载对应平台的构建产物。

### Windows

运行 `CodexTimeZoneLauncher.exe`。程序会在可执行文件旁的 `data` 目录保存设置，因此整个目录可以便携移动。

### macOS

打开构建生成的 `.app`。首次分发到其他设备前，应按 Apple 的要求完成代码签名和公证。

## 从源码构建

### 环境要求

- Node.js 22.12 或更高版本、npm 10 或更高版本
- Rust stable 工具链
- Windows：Visual Studio Build Tools、Windows SDK、WebView2 Runtime
- macOS：Xcode Command Line Tools

### Windows

```powershell
cd resource
npm ci --cache ../environment/npm-cache
npm run dev:win
```

生成 Release 版本：

```powershell
cd resource
npm run build:win
```

如开发依赖安装在自定义目录，可先设置：

```powershell
$env:CODEX_TZ_DEV_ROOT = "E:\development"
```

### macOS

```bash
cd resource
npm ci --cache ../environment/npm-cache
npm run dev:mac
```

生成 Release 版本：

```bash
cd resource
npm run build:mac
```

只构建前端界面：

```bash
cd resource
npm run build:frontend
```

## 配置与隐私

- Windows 配置保存在程序目录旁的 `data` 文件夹。
- macOS 配置保存在 `~/Library/Application Support/com.fei-away.codextimezone`。
- 项目不要求登录账户，也不会修改系统全局时区。
- 公网 IP 功能会按所选线路请求第三方 IP/定位服务；不使用该功能时可不刷新网络信息。
- 复制 IP 使用系统剪贴板，并在写入后进行结果校验。

## 项目结构

```text
.
├─ CodexTimeZoneLauncher.exe       # Windows 便携版产物
├─ Codex 时区启动器.app/           # macOS 应用产物
├─ README.md
└─ resource/
   ├─ backend/                     # 公共逻辑及 Windows、macOS 平台实现
   ├─ src/                         # Vue 页面、组件、composables 与样式
   ├─ src-tauri/                   # Tauri/Rust 应用入口与配置
   ├─ desktop.mjs                  # 跨平台开发、构建和启动脚本
   └─ package.json
```

## 技术栈

- Vue 3、TypeScript、Vite
- MacVue UI
- Tauri 2、Rust
- 平台原生脚本与应用启动能力

## 当前验证状态

- 前端生产构建已通过。
- Windows x64 Release 构建已通过，根目录可执行文件已更新。
- 主要界面已按深色/浅色主题及多个桌面窗口尺寸完成截图验收。
- 最新 macOS 构建、Windows 真实 Codex 启动、快捷方式、系统剪贴板和 Dream Skin 联动仍建议在发布前做一次目标机器回归。

## License

本项目采用 [MIT License](LICENSE)。

> 本项目是独立开源项目，并非 OpenAI 官方发行或支持的软件。Codex 是 OpenAI 的产品名称。
