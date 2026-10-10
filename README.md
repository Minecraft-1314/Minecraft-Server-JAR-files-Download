# Minecraft Server Downloader 0.0.1

## 重要公告（必读）

- 本项目面向 Windows 10/11 x64。
- 默认版需要本机安装 .NET Desktop Runtime 8；便携版已内置 .NET 运行时。
- 所有版本都需要 Microsoft Edge WebView2 Runtime。
- 安装程序尚未使用代码签名证书，Windows SmartScreen 可能显示安全提示。
- 正常的版本清单导入只填写名称和版本清单地址，不收集账号、密码、令牌或用户 ID。
- 请勿在日志、问题反馈或截图中粘贴任何私密信息。

## 项目介绍

这是一个面向 Windows 的 Minecraft 正式版服务器 JAR 下载工具，采用 **WPF 外壳 + WebView2 前端 + C# 核心库 + 静态前端资源**的分层结构，支持官方 Mojang 清单、BMCLAPI 镜像和自定义版本清单导入、版本搜索、批量选择、并发下载、暂停继续、停止取消、断点续传、SHA-1 校验、磁盘空间预检、中英文界面以及深色和浅色主题。

版本清单中的 `id`、`type`、`url`、`time`、`releaseTime` 和 `sha1` 等字段由程序从结构化 JSON 中读取，不需要用户手工填写。

## 下载与安装

| 产物 | 用途 | 运行条件 |
|------|------|------|
| `MinecraftServerDownloader-Setup.exe` | Inno Setup 7.1.0 标准安装向导，支持中文和英文 | Windows 10/11 x64，安装时需要管理员权限 |
| `MinecraftServerDownloader.msi` | 标准 Windows Installer 安装包 | Windows 10/11 x64，安装时需要管理员权限 |
| `MinecraftServerDownloader-default-win-x64.zip` | 默认版压缩包 | 需要 .NET Desktop Runtime 8 和 WebView2 Runtime |
| `MinecraftServerDownloader-portable-win-x64.zip` | 单文件便携版压缩包 | 已内置 .NET 运行时，仍需要 WebView2 Runtime |

推荐普通用户使用 `MinecraftServerDownloader-Setup.exe`。安装和卸载使用标准 Inno Setup / Windows Installer 流程，不使用自定义 WPF 安装界面。

## 核心功能

- 获取 Mojang 官方正式版清单，支持 BMCLAPI 镜像和自定义版本清单地址。
- 按版本号搜索、全选未下载、清空选择，并识别目标目录中已下载的 JAR。
- 最多 8 个并发下载任务，支持暂停、继续、停止、取消和断点续传。
- 下载完成后按官方 SHA-1 校验，失败文件不会被当作有效结果保留。
- 下载前检查磁盘空间，下载日志限制为最近 300 条。
- 网络请求包含超时、取消、有限重试、重定向复核和 SSRF/内网地址防护。
- 首次启动只创建内存默认值，保存设置或导入来源前不写入配置。
- 损坏的配置或缓存会被安全忽略并显示可恢复状态。

## 构建

在开发环境目录执行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\build.ps1
```

构建脚本按顺序执行 Release 构建、核心测试、CSS 构建、翻译键审计、UI 几何审计、默认发布、单文件便携发布、卸载启动器发布、ZIP、WiX MSI 和 Inno Setup 编译，并校验图标尺寸与成品清单。

## 项目结构

```text
MinecraftServerDownloader.csproj       WPF 外壳入口
Program.cs                              进程启动与全局异常处理
Ui/WebViewWindow.cs                     WebView2 初始化、原生回退和消息桥接
Ui/Bridge/                              JSON 协议、操作闸门、状态视图和命令路由
src/MinecraftServerDownloader.Core/     清单、网络、下载、缓存、配置、安全校验
Ui/Web/                                 本地 HTML、Tailwind CSS、JavaScript 和图标
installer/                              Inno Setup 与 WiX MSI 源文件
Uninstaller/                            标准卸载流程启动器
```

## 项目贡献者

| 贡献者 | 贡献内容 |
|------|------|
| Minecraft-1314 | 完整开发 |

欢迎通过 Issues 或 PR 加入贡献者列表。

## 许可协议

本项目采用 MIT 许可证，详情参见 [LICENSE](LICENSE) 文件。

## 支持我们

如果这个项目对您有帮助，欢迎点亮 GitHub 仓库右上角的 Star。
