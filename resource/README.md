# 源码说明

完整功能和使用说明见仓库根目录的 [README.md](../README.md)。

`backend/win` 和 `backend/mac` 分别保存两个平台的 Rust 后端；`backend/common.rs` 和 `backend/zones.json` 为共用逻辑与时区数据。`src` 是共用 Vue 前端，`src-tauri` 是桌面外壳。`desktop.mjs` 是共用 npm 构建与运行调度入口，不属于业务后端；Windows 专用 PowerShell 环境脚本放在 `backend/win`，由该入口自动调用。

环境目录统一在根目录 `../environment`，源码目录不再生成同名文件夹。构建缓存使用按脚本位置解析的绝对路径。直接执行 `npm ci` 使用 npm 默认用户缓存；要集中安装缓存，可执行 `npm ci --cache ../environment/npm-cache`。

在本目录执行 `npm ci`，然后使用：

| 操作 | macOS | Windows |
| --- | --- | --- |
| 开发运行 | `npm run dev:mac` | `npm run dev:win` |
| 生产构建 | `npm run build:mac` | `npm run build:win` |
| 启动成品 | `npm run start:mac` | `npm run start:win` |

构建产物输出到仓库一级目录。另一端源码和专用依赖通过 Rust 条件编译排除。macOS 开发工具可使用 Homebrew 安装 Node.js 和 Rust。

`npm run dev:frontend` / `npm run build:frontend` 仅启动或构建前端，由 Tauri 自动调用。单独预览前端没有原生后端。

界面使用 MacVue 的弹出菜单、按钮、复选框和玻璃面板。网络面板通过系统代理环境查询代理出口和直连出口的公网 IP 与时区；网络不可用时分别显示未配置或查询失败，不会修改已有时区设置。

可选环境变量：`CODEX_TZ_DEV_ROOT` 指定 Windows 开发工具目录，`CODEX_TZ_PROXY` 指定下载代理。macOS 构建会自动加入 Homebrew 工具路径。
