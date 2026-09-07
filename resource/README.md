# 源码说明

完整项目说明、截图和使用方法见仓库根目录的 `README.md`。

该目录包含 Vue 3 前端、Tauri 2 桌面外壳及 C# 系统功能后端。构建脚本支持两个可选环境变量：

- `CODEX_TZ_DEV_ROOT`：自定义 Node.js、pnpm、Git、Rust 和 Visual Studio Build Tools 的集中安装目录。
- `CODEX_TZ_PROXY`：依赖下载使用的 HTTP/HTTPS 代理地址。

未设置这些变量时，脚本使用系统 `PATH` 和 Visual Studio Build Tools 的标准安装目录。
