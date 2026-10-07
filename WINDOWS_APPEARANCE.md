# Windows 图标、RTSS 与亚克力调查

## 图标

原项目 ICO 的内容已解码确认是时钟图标。此前仅设置 runner 的窗口类图标；这不能保证读取 Flutter 子窗口或小图标的外部工具得到同一图标，也无法排除旧程序或 Explorer 缓存。

现在将原 ICO 的大小图标显式设置到 runner 与 Flutter 子窗口的 WM_SETICON 和窗口类属性，并更新 EXE 的中文产品描述。测试进程通过 WM_GETICON 读取的小图标已确认为原时钟图标。请退出旧版后从新包启动；固定到任务栏的旧快捷方式若仍缓存图标，可取消固定后重新固定。

## MSI Afterburner / RTSS

截图中的 CPU/GPU/RAM/D3D11 是第三方性能叠加层。如果 RTSS 全局配置启用了图形程序检测，Flutter 的硬件加速渲染可能进入其检测范围。

可使用 RTSS SDK 的 LoadProfile / SetProfileProperty / SaveProfile / UpdateProfiles，为 **codex_timezone.exe** 单独设置 AppDetectionLevel=0、EnableOSD=0，无需关闭 Afterburner 或调整游戏配置。

可在其他机器执行（PowerShell 7，传入实际 RTSS 安装目录）：

```powershell
.\scripts\disable-rtss-overlay.ps1 -RtssDirectory '<RTSS 安装目录>'
```

脚本备份位于 `environment/rtss-backup-<时间>`。撤销时可在 RTSS 应用列表删除 codex_timezone.exe 项；原先已有配置则可从备份恢复。

正在运行的旧启动器需要退出并重开。测试进程仍可枚举到 RTSS 模块，因此不能把“专属配置关闭”写成“完全没有注入”。最终是否不再显示 OSD，需要在可见桌面确认；如果仍显示，在 RTSS 中选择本应用确认 Application detection level 为 None、On-Screen Display support 为 Off。此设置针对本机，不随 ZIP 自动应用到其他电脑。

## 半透明亚克力

使用锁定版本 flutter_acrylic 1.1.4。Windows 11 使用 DWM Desktop Acrylic，页面背景透明，卡片使用半透明填充；正文、菜单和输入控件保持清晰。明暗主题变化同步到原生材质。此实现没有降低整个窗口的透明度。

透明填充限定在 `cardStyle`，不再降低全局 `colors.card` 的不透明度。Forui 0.27.0 的选择菜单、弹出层与可用输入框会复用该全局颜色，因此必须保留实色；只有页面卡片使用浅色 54% / 深色 48% 不透明度。搜索框及菜单项可以透出其父浮层，但父浮层本身必须不透明。

原生属性检查读到 DWMWA_SYSTEMBACKDROP_TYPE=3，即 Desktop Acrylic。材质受系统“透明效果”、高对比度及系统合成策略影响；原生属性验证不等同于所有桌面场景的视觉验收。Windows 10 的插件兼容路径仍需验证。

插件初始化失败时使用实色主题；手动禁用材质：

```powershell
.\codex_timezone.exe --solid-background
```

## 依据

- [Flutter Windows 图标与 runner 文档](https://docs.flutter.dev/platform-integration/windows/building)
- [Microsoft DWM 背景材质定义](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwm_systembackdrop_type)
- [flutter_acrylic 官方项目与平台限制](https://github.com/alexmercerind/flutter_acrylic)
- RTSS 安装目录中的 `SDK/Samples/SharedMemory/RTSSSharedMemorySample/RTSSProfileInterface.h`（SDK 原始接口说明）。
