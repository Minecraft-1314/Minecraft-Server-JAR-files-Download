# Minecraft Server Downloader

## 重要公告（必读）

- 本项目面向 Windows 10/11 x64。
- 所有版本都需要 Microsoft Edge WebView2 Runtime。
- 安装程序尚未使用代码签名证书，Windows SmartScreen 可能显示安全提示。
- 请勿在日志、问题反馈或截图中粘贴任何私密信息。

## 项目介绍

这是一个面向 Windows 的 Minecraft 正式版服务器 JAR 下载工具，采用 **Rust 原生窗口外壳 + WebView2 前端 + Rust 核心库 + 静态前端资源**的分层结构，支持官方 Mojang 清单、BMCLAPI 镜像和自定义版本清单导入、版本搜索、批量选择、并发下载、暂停继续、停止取消、断点续传、SHA-1 校验、磁盘空间预检、中英文界面以及深色和浅色主题。

## 下载与安装

| 产物 | 用途 | 运行条件 |
|------|------|------|
| `MinecraftServerDownloader-Setup.exe` | Inno Setup 标准安装向导，支持中文和英文 | Windows 10/11 x64，安装时需要管理员权限 |
| `MinecraftServerDownloader.msi` | 标准 Windows Installer 安装包 | Windows 10/11 x64，安装时需要管理员权限 |
| `MinecraftServerDownloader-portable-win-x64.zip` | 便携版压缩包，解压即用 | Windows 10/11 x64，需要 WebView2 Runtime |
| `dist/MinecraftServerDownloader-win-x64` | 免安装目录版 | Windows 10/11 x64，需要 WebView2 Runtime |

推荐普通用户使用 `MinecraftServerDownloader-Setup.exe`。

## 项目贡献者

| 贡献者 | 贡献内容 |
|------|------|
| Minecraft-1314 | 完整开发 |

欢迎通过 Issues 或 PR 加入贡献者列表。

## 许可协议

本项目采用 MIT 许可证，详情参见 [LICENSE](LICENSE) 文件。

## 支持我们

如果这个项目对您有帮助，欢迎点亮 GitHub 仓库右上角的 Star。
