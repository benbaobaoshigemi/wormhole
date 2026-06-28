# AGENTS.md

## 说明

这是 Codex 在本仓库中的工作指令文件。

开始任何任务前，先阅读：

- `project.md`：产品需求、功能边界、架构方向、UI 目标。
- 本文件：开发约定、仓库规则、验证要求。

不要把 `AGENTS.md` 当成产品需求文档。产品定义以 `project.md` 为准。

---

## 工作方式

- 先理解现有代码结构，再修改。
- 优先做最小、可验证的改动。
- 不要一次性重写无关模块。
- 不要把临时验证代码混入正式模块。
- 不要擅自扩大产品功能范围。
- 不确定时，先在说明中列出假设，再实现。
- 修改完成后，说明改了什么、怎么验证、还有什么风险。

---

## 项目边界

本项目是两台固定桌面电脑之间的局域网互通工具。

首发目标是 Windows ↔ macOS，后续预留 Windows ↔ Windows。

不要实现以下内容：

- 手机端
- 多设备管理
- 自动局域网发现
- 云端中转
- 公网穿透
- 远程桌面
- 键鼠穿越
- 文件列表剪贴板
- Office / HTML / RTF / PDF 等复杂剪贴板格式
- 与文件穿越、文本/图片剪贴板互通无关的功能

---

## 架构约定

保持 Core、Platform Adapter、UI 三层边界。

Core 负责：

- 连接状态
- 文件传输
- 文件夹传输
- 传输队列
- 失败重试
- 历史记录
- 剪贴板同步状态
- 配置
- 事件输出

Platform Adapter 负责：

- Windows / macOS 剪贴板
- Windows / macOS 拖拽与边缘窗口
- 系统通知
- 托盘 / 菜单栏
- 开机启动
- 平台权限处理

UI 负责：

- 主窗口
- 设置页
- 传输队列展示
- 历史记录展示
- 边缘投递 UI
- 接收 / 传输浮层
- 用户可见文案和视觉表现

不要把 Core 逻辑写进 UI。

不要让 Core 直接依赖 Win32、Cocoa、AppKit 或具体 UI 框架。

---

## 技术方向

优先采用：

- Rust 作为核心功能主要实现语言。
- Tauri 作为桌面 UI 壳的优先方向。
- SQLite 保存传输历史、任务状态和必要运行数据。
- 本地 API / 事件流连接 Core 与 UI。

可以根据实际情况调整实现细节，但必须保留 UI 与核心功能解耦的结构。

---

## Windows 优先开发说明

当前开发环境首先在 Windows 主机上推进。

可以先用本机双节点模拟验证核心链路，但不要让代码依赖“两个节点在同一台机器”这一特殊条件。

Windows 阶段完成的 Core 代码，应为后续 macOS 适配保留平台无关结构。

---

## 剪贴板约定

只处理系统通用内容：

- 文本
- URL 按文本处理
- 图片
- 截图图片

剪贴板同步必须防回环。

不要在日志、历史记录或 UI 中显示剪贴板正文。

图片剪贴板应使用统一中间格式传输，平台格式转换放在 Platform Adapter 中。

---

## 文件传输约定

文件传输必须支持：

- 单文件
- 多文件
- 文件夹
- 大文件
- 进度
- 队列
- 失败重试
- 历史记录

文件夹传输应保留相对目录结构。

接收端应避免半截文件污染最终目录。

不得把大文件整体读入内存。

---

## UI 约定

UI 必须向消费级软件靠拢。

不要做成工程控制台、网页后台或参数面板。

首页重点表达：

- 是否已连接
- 对端是谁
- 文件穿越是否可用
- 剪贴板互通是否开启
- 当前是否有传输任务

复杂设置、日志和诊断信息放到二级页面。

UI 可以阶段性粗糙，但不得承载核心业务逻辑。

---

## 构建与运行

仓库形成实际工程后，在这里维护最新命令。

如果命令发生变化，修改代码的同时更新本节。

```powershell
# 正式 MVP：初始化 Windows / macOS 配置模板
.\scripts\init-mvp-configs.ps1 -MacHost 192.168.1.180

# 正式 MVP：构建 Core Daemon + CLI
cmd /s /c "call ""C:\Program Files\Microsoft Visual Studio\18\Community\Common7\Tools\VsDevCmd.bat"" -arch=x64 -host_arch=x64 && ""C:\Users\zhang\.cargo\bin\cargo.exe"" build --release -p wormhole-daemon -p wormhole-cli"

# 正式 MVP：启动 Windows 节点
.\target\release\wormhole-daemon.exe --config .\.wormhole\windows\config.json

# 正式 MVP：部署 / 构建 macOS 节点
C:/Users/zhang/miniconda3/python.exe .\scripts\deploy-macos.py 192.168.1.180

# 正式 MVP：启动 macOS 节点
C:/Users/zhang/miniconda3/python.exe .\scripts\run-macos-daemon.py 192.168.1.180

# 正式 MVP：CLI 操作
.\target\release\wormhole-cli.exe --api http://127.0.0.1:53317 connect
.\target\release\wormhole-cli.exe --api http://127.0.0.1:53317 send <file-or-folder>
.\target\release\wormhole-cli.exe --api http://127.0.0.1:53317 tasks
.\target\release\wormhole-cli.exe --api http://127.0.0.1:53317 cancel <task-id>
.\target\release\wormhole-cli.exe --api http://127.0.0.1:53317 retry
.\target\release\wormhole-cli.exe --api http://127.0.0.1:53317 clear-history
.\target\release\wormhole-cli.exe --api http://127.0.0.1:53317 clipboard-text
.\target\release\wormhole-cli.exe --api http://127.0.0.1:53317 clipboard-image

# 正式 MVP：最小 UI
# 浏览器打开 http://127.0.0.1:53317/

# 技术验证：一键跑完整双节点验证
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py run-all

# 技术验证：Windows ↔ macOS 真机联调
C:/Users/zhang/miniconda3/python.exe _verification_scripts/macos_link_validation.py

# 技术验证：剪贴板专项真机联调
C:/Users/zhang/miniconda3/python.exe _verification_scripts/clipboard_validation.py --remote-host 192.168.1.180

# 技术验证：初始化 Node A / Node B 配置
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py init

# 技术验证：启动单个节点
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py serve --config _verification_runtime/A/config/node.json
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py serve --config _verification_runtime/B/config/node.json

# 技术验证：查看状态、连接和事件
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py status --node A
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py connect --node A
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py events --node A

# 技术验证：发送文件 / 文件夹
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py send-file --node A <file>
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py send-folder --node A <folder>

# 技术验证：剪贴板
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py clipboard-text --node A "hello"
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py clipboard-image --node A <png>
```

不要提交无法运行且未说明原因的命令。

---

## 测试要求

修改 Core 后，应尽量补充或更新测试。

优先覆盖：

- 配置加载
- 握手状态
- 文件 manifest
- 文件夹相对路径
- 传输状态流转
- 失败重试
- 剪贴板 hash 防回环
- 图片大小限制
- 平台适配边界

涉及平台能力的代码，至少提供可手动复现的验证步骤。

---

## 日志与隐私

日志可以记录：

- 错误类型
- 连接状态
- 传输状态
- 文件名
- 文件大小
- 系统 API 错误

日志不得记录：

- 剪贴板正文
- 图片二进制内容
- 文件内容
- 认证 token
- 用户隐私数据

---

## 提交前检查

提交或完成任务前，确认：

- 没有引入与 `project.md` 冲突的功能。
- 没有把 Core 逻辑塞进 UI。
- 没有把平台 API 泄漏到 Core。
- 没有提交机器私有路径、密钥或临时垃圾文件。
- 新增命令、配置或运行方式已经写清楚。
- 已说明验证方式和剩余风险。

---

## 回复格式

完成开发任务后，回复应包含：

- 本次改动
- 关键文件
- 验证命令
- 验证结果
- 已知问题
- 后续建议

不要只回复“已完成”。

没有实际运行验证时，必须明确说明。
