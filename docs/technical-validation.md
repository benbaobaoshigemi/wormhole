# 技术验证报告

## 结论

本次技术验证已经证明：在一台 Windows 主机上，用两个独立节点模拟两台固定桌面电脑时，本项目的关键链路成立，可以进入正式软件开发阶段。

本阶段没有开发正式 UI、边缘投递 UI、托盘、自动发现、键鼠穿越、手机端或多设备管理。

补充真机联调结论：Windows 主机与 macOS Air 之间的局域网联调也已通过。Windows 使用 `C:\Users\zhang\Desktop\hole`，macOS 使用 `~/Desktop/hole`，未在其它本地/远端目录写入验证文件。

## 本次实现

- 新增临时验证器：`_verification_scripts/wormhole_validation.py`
- 新增 Windows ↔ macOS 真机联调器：`_verification_scripts/macos_link_validation.py`
- 新增本地参考 fork：
  - LocalSend：`_verification_scripts/reference_forks/localsend`，commit `5ccc6dea192d1c697c2602bf456b2eb2ad8e9674`
  - Deskflow：`_verification_scripts/reference_forks/deskflow`，commit `570c9a494b53bfa2d2291cb0565d71aef49e3b5e`
- 新增运行时目录：`_verification_runtime/`
  - Node A / Node B 分别有独立配置、端口、接收目录和日志目录。
  - 该目录是验证产物，已加入 `.gitignore`。

参考 fork 的作用是作为本地可追溯证据源。验证器没有把 LocalSend 或 Deskflow 的代码混进主实现；它只吸收了以下路线：

- LocalSend 风格的 prepare + upload 文件传输流程。
- Deskflow 风格的连接状态、剪贴板变化事件和跨设备剪贴板同步思路。

## 如何运行

一键完整验证：

```powershell
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py run-all
```

Windows ↔ macOS 真机联调：

```powershell
C:/Users/zhang/miniconda3/python.exe _verification_scripts/macos_link_validation.py
```

该脚本会在运行时提示输入 macOS SSH 密码。密码不会写入文件、配置或报告。

剪贴板专项真机联调：

```powershell
C:/Users/zhang/miniconda3/python.exe _verification_scripts/clipboard_validation.py --remote-host 192.168.1.180
```

如果 `Air.local` 可以稳定解析，也可以省略 `--remote-host`。

初始化双节点配置：

```powershell
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py init
```

分别启动节点：

```powershell
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py serve --config _verification_runtime/A/config/node.json
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py serve --config _verification_runtime/B/config/node.json
```

常用 CLI：

```powershell
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py status --node A
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py connect --node A
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py send-file --node A <file>
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py send-folder --node A <folder>
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py events --node A
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py clipboard-text --node A "hello"
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py clipboard-image --node A <png>
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py retry --node A
```

真实 Windows 文本剪贴板最小验证入口：

```powershell
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py windows-clipboard-read-send-text --node A
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py windows-clipboard-write-text --node B "模拟对端 -> Windows真实剪贴板"
```

日志位置：

```text
_verification_runtime/A/logs/events.jsonl
_verification_runtime/B/logs/events.jsonl
_verification_runtime/A/logs/process.stdout.log
_verification_runtime/B/logs/process.stdout.log
```

## 能力验证结果

### 1. 本地双节点模拟

已通过。

- Node A：`127.0.0.1:54101`
- Node B：`127.0.0.1:54102`
- 双方使用不同 device_id、配置目录、接收目录和日志目录。
- 协议使用 host/port，不依赖两个节点在同一台机器这一特殊事实。

### 2. 握手与连接状态

已通过。

实际验证：

- A 能连接 B。
- B 能连接 A。
- 双方识别对端 device_id、device_name、platform、protocol_version。
- 错误端口会失败。
- B 关闭后，A 重新连接得到 `ConnectionRefusedError(10061)`。
- B 重启后，A 能重新握手成功。

当前握手保留了后续加入 token / 配对校验的位置，但本阶段没有实现完整安全体系。

### 3. 文件传输

已通过。

覆盖：

- 小文本文件。
- 二进制文件。
- 中文文件名。
- 带空格文件名。
- 16 MB 大文件流式传输。

发送端按 1 MB chunk 读取并发送；接收端按 chunk 写入。验证器没有一次性把大文件整体读入内存。

接收端先写入：

```text
<filename>.wormhole_tmp
```

完成后再 rename 到最终文件。

### 4. 文件夹传输

已通过。

覆盖：

- 多层目录。
- 空文件。
- 中文目录名。
- 多个文件。
- 相对目录结构保持。

验证器会扫描目录生成 manifest，然后按相对路径逐文件上传。

### 5. 传输进度与事件

已通过。

事件以 JSONL 输出，内容表达事实，不写 UI 文案。

已覆盖事件：

```text
connection.changed
transfer.created
transfer.started
transfer.progress
transfer.completed
transfer.failed
transfer.retrying
clipboard.synced
clipboard.ignored
clipboard.too_large
```

### 6. 失败与重试

已通过。

覆盖：

- 接收节点未启动。
- 端口错误。
- 传输中断。
- 接收目录不可用。
- 重试入口。

传输中断通过接收端调试注入实现：接收端在收到 1 MB 后主动中断。验证结果显示最终文件不存在，半截内容没有污染最终路径。

### 7. 剪贴板文本验证

已通过。

内存剪贴板模型覆盖：

- A 设置文本，B 收到。
- B 设置文本，A 收到。
- 相同内容不会无限循环。
- hash 可阻止回环。

真实 Windows 文本剪贴板最小验证覆盖：

- Windows 真实剪贴板 -> 模拟对端。
- 模拟对端 -> Windows 真实剪贴板。

注意：当前真实剪贴板接入使用 PowerShell `Get-Clipboard` / `Set-Clipboard`，只用于技术验证。正式版应在 Windows Platform Adapter 中使用 Win32 Clipboard API。

### 8. 剪贴板图片验证

已通过路线验证。

覆盖：

- PNG 文件作为统一图片载荷。
- 图片 payload 从 A 发送到 B。
- 图片 hash 防回环模型。
- 图片大小限制。
- 超过限制时输出 `clipboard.too_large` 事件并忽略。

当前没有接入真实 Windows 图片剪贴板。剩余风险：

- Windows CF_DIB / CF_DIBV5 到 PNG 的转换。
- PNG 写回 Windows 图片剪贴板。
- 截图工具、微信截图、浏览器复制图片等来源格式差异。
- 大图片转换时的内存峰值控制。

正式开发时这部分应放入 Windows Platform Adapter，Core 只处理 PNG bytes 和大小限制。

### 9. 命令行或脚本入口

已提供。

`wormhole_validation.py` 可完成：

- 启动 Node A。
- 启动 Node B。
- 查看节点状态。
- 测试连接。
- 发送文件。
- 发送文件夹。
- 查看传输事件。
- 模拟文本剪贴板。
- 模拟图片剪贴板。
- 查看日志和自动验证结果。

### 10. 实际验证结果

最终命令：

```powershell
C:/Users/zhang/miniconda3/python.exe _verification_scripts/wormhole_validation.py run-all
```

最终结果：

```json
{
  "ok": true,
  "root": "C:\\Users\\zhang\\Desktop\\hole\\_verification_runtime"
}
```

关键通过项：

- `A connects B`
- `B connects A`
- `port error fails`
- `send text`
- `send binary`
- `send chinese`
- `send spaced`
- `send large`
- `send folder`
- `clipboard text A to B`
- `clipboard text B to A`
- `windows clipboard to simulated peer`
- `simulated peer to windows clipboard`
- `clipboard image png`
- `clipboard image size limit`
- `transfer interruption fails`
- `interrupted final file absent`
- `retry after interruption`
- `peer shutdown detected`
- `peer restart reconnects`
- `retry after restart`
- `receive dir unavailable fails`
- `event coverage`

完整机器可读结果位于：

```text
_verification_runtime/validation-result.json
```

## Windows ↔ macOS 真机联调结果

最终命令：

```powershell
C:/Users/zhang/miniconda3/python.exe _verification_scripts/macos_link_validation.py
```

本地与远端目录：

```text
Windows: C:\Users\zhang\Desktop\hole
macOS:   /Users/benbaobaoshigemi/Desktop/hole
```

最终结果：

```json
{
  "ok": true,
  "local_root": "C:\\Users\\zhang\\Desktop\\hole\\_verification_runtime_macos",
  "remote_root": "/Users/benbaobaoshigemi/Desktop/hole"
}
```

已通过：

- Windows A 连接 macOS B。
- macOS B 连接 Windows A。
- Windows 发送文件到 macOS。
- Windows 发送文件夹到 macOS，并保持中文目录、多层目录和空文件。
- macOS 发送文件到 Windows。
- Windows 剪贴板模型同步到 macOS。
- macOS 剪贴板模型同步到 Windows。
- Windows 侧事件覆盖连接、传输进度、传输完成和剪贴板同步。
- macOS 节点关闭后，Windows 能检测连接失败。
- macOS 节点重启后，Windows 能重新连接。

机器可读结果位于：

```text
_verification_runtime_macos/macos-link-result.json
```

## 剪贴板专项真机联调结果

最终命令：

```powershell
C:/Users/zhang/miniconda3/python.exe _verification_scripts/clipboard_validation.py --remote-host 192.168.1.180
```

最终结果：

```json
{
  "ok": true,
  "local_root": "C:\\Users\\zhang\\Desktop\\hole\\_verification_runtime_clipboard",
  "remote_root": "/Users/benbaobaoshigemi/Desktop/hole"
}
```

已通过：

- Windows 剪贴板节点连接 macOS。
- macOS 剪贴板节点连接 Windows。
- Windows 真实文本剪贴板 -> macOS 真实文本剪贴板。
- macOS 真实文本剪贴板 -> Windows 真实文本剪贴板。
- 中文文本跨平台不乱码。
- 接收端刚写入远端文本后，再监听到本机剪贴板变化时会被 hash 防回环拦截。
- Windows 真实图片剪贴板 -> PNG 统一载荷 -> macOS 真实图片剪贴板。
- macOS 真实图片剪贴板 -> PNG 统一载荷 -> Windows 真实图片剪贴板。
- 图片剪贴板防回环。
- 图片大小限制事件 `clipboard.too_large`。
- 剪贴板事件覆盖 `clipboard.synced`、`clipboard.ignored`、`clipboard.too_large`。

机器可读结果位于：

```text
_verification_runtime_clipboard/clipboard-validation-result.json
```

### 参考项目结论

本次剪贴板专项没有把验证器视为最终方案，而是重新对照了本地参考项目：

- Deskflow 使用 `IClipboard` 抽象隔离平台剪贴板，Core 不直接碰 Win32 / AppKit。
- Deskflow 的剪贴板 payload 使用 marshall/unmarshall 结构：格式数量、格式 ID、payload size、payload bytes。
- Deskflow 有明确事件：`ClipboardGrabbed`、`ClipboardChanged`、`ClipboardSending`。
- Deskflow 对剪贴板大 payload 使用 `StreamChunker` 分块，默认 chunk 为 512 KB。
- LocalSend 继续作为 HTTP prepare/upload 和 stream 传输参考，不作为剪贴板主参考。

对正式开发的影响：

- 正式 Core 应采用 Deskflow 类似的 ClipboardPort / payload / event 模型。
- 正式图片剪贴板不要依赖 UI 或控制台命令，应由 Windows/macOS Platform Adapter 负责 PNG 与系统格式转换。
- 正式事件应表达事实，例如 `clipboard.synced`、`clipboard.ignored`、`clipboard.too_large`，不要写 UI 文案。
- 正式传输大图片时应避免 Base64 JSON，改成二进制 body 或分块流。
- 系统剪贴板 hash 应基于“从系统剪贴板抽取后的规范化载荷”，不能假设源 PNG 文件字节与系统重编码后的 PNG 字节一致。

## 当前未解决的问题

- 没有实现正式 Rust Core Daemon。
- 没有实现正式 HTTP + WebSocket API。
- 没有 SQLite 历史记录。
- 没有正式队列、取消、限速、续传。
- 没有 token / pairing / 局域网最小认证。
- 没有真实 Windows 图片剪贴板适配。
- 没有真实 macOS 系统剪贴板适配；本次真机只验证了 macOS 进程、局域网通信、文件传输和剪贴板统一载荷模型。
- 剪贴板专项已经验证真实 Windows/macOS 文本剪贴板和真实 Windows/macOS 图片剪贴板，但实现仍是临时脚手架；正式版必须改为 Rust Platform Adapter。
- 没有 Tauri UI、托盘、边缘投递 UI。
- 当前事件 API 是内存窗口，正式版需要事件总线加持久日志。

## 建议保留的代码

正式开发时建议保留为验证资产：

- `_verification_scripts/wormhole_validation.py`
  - 用作协议雏形、失败注入、回归验证脚本。
- `_verification_scripts/reference_forks/localsend`
  - 用作文件传输流程参考。
- `_verification_scripts/reference_forks/deskflow`
  - 用作剪贴板事件和连接状态参考。
- `docs/technical-validation.md`
  - 用作正式 Core 开发前的技术边界文档。

## 建议丢弃或重写的临时代码

正式产品中不要直接使用：

- Python HTTP server。
- PowerShell 剪贴板桥接。
- macOS `pbcopy/pbpaste/osascript` 剪贴板桥接。
- Base64 JSON 图片传输。
- 历史阶段曾使用 `/api/debug/fail-next-upload-after` 调试端点；当前正式 Rust 原型已移除 `/api/*` 命名空间。
- 内存任务表。
- 内存事件窗口。
- 当前简化 manifest 格式。

这些代码的价值是证明链路，不是成为最终架构。

## 正式开发建议

下一阶段建议从 Rust Core Daemon 开始：

1. 建立 `crates/wormhole-core`、`wormhole-daemon`、`wormhole-platform`、`wormhole-cli`。
2. 先迁移本验证中的协议模型：handshake、prepare-transfer、stream upload、tmp rename、event bus。
3. 引入 SQLite 保存 transfer_tasks、transfer_items、history、clipboard_events。
4. 在 Core 中定义 `ClipboardPort`，Windows 文本和图片剪贴板放到 Platform Adapter。
5. 给 HTTP 命令接口和事件流建立稳定 schema。
6. 加最小 shared token，避免局域网内误调用。
7. UI 最后接入 Core API，不让 UI 承载传输、剪贴板、队列或历史逻辑。

## 对正式软件开发的结论

项目关键技术链路成立。

文件/文件夹传输、流式大文件、临时文件落盘、失败状态、重试入口、连接状态、文本剪贴板模型、PNG 图片载荷模型、图片大小限制和防回环机制都已经可复现验证。

正式开发可以开始，但应把本阶段 Python 代码视为验证脚手架，而不是产品代码。
