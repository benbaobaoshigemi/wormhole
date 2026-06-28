# 局域网双电脑互通应用项目文档

## 1. 产品定位

本应用是一款面向两台固定桌面电脑的局域网互通工具，用于在两台电脑之间快速传输文件、文件夹，并自动同步系统剪贴板。

首发目标场景：

- Windows ↔ macOS

后续预留场景：

- Windows ↔ Windows

暂不考虑：

- 手机端
- 多设备同时互通
- 账号系统
- 云端中转
- 公网穿透
- 远程桌面
- 键鼠穿越
- 目录自动同步
- 聊天功能
- 剪贴板历史管理器

产品目标：

- 用户拖文件或文件夹到屏幕边缘，即可发送到另一台电脑。
- 用户复制文字或截图图片，另一台电脑自动获得相同剪贴板内容。
- 应用保持后台低资源占用，传输快速稳定，UI 达到消费级软件质感。

---

## 2. 核心功能

应用只包含两个核心功能：

1. 文件穿越
2. 自动剪贴板互通

文件穿越负责主动传输文件和文件夹。

剪贴板互通负责自动双向同步文字和图片剪贴板。

两者互相独立，可分别开启或关闭。

---

## 3. 设备与连接模型

### 3.1 设备范围

应用面向两台固定电脑。

每台电脑都运行同一个客户端。

每台设备记录以下信息：

- 设备名称
- 系统类型：Windows / macOS
- 设备 ID
- 本机 IP
- 监听端口
- 对端 IP
- 对端端口
- 连接状态
- 接收目录
- 触发边缘配置

### 3.2 首次配置

首次启动时，用户手动配置对端 IP 和端口。

不做局域网自动发现。

不做广播扫描。

不做二维码配对。

### 3.3 自动连接

自动连接需要支持。

自动连接的含义：

- 首次手动配置对端 IP 和端口。
- 之后应用启动时自动尝试连接。
- 断开后自动重连。
- 对端休眠、关机、离线时显示离线状态。
- 对端恢复后自动连接。

### 3.4 在线状态

连接状态至少包含：

- 未配置
- 正在连接
- 已连接
- 对端离线
- 连接失败
- 传输中

首页必须清楚显示当前状态。

---

## 4. 文件穿越功能

### 4.1 基本流程

用户在发送端拖动一个或多个文件或文件夹。

拖动到预设屏幕边缘。

边缘弹出接纳 UI。

用户松手。

发送端创建传输任务，加入传输队列。

发送端与接收端握手。

接收端边缘弹出接收 UI。

双方显示传输进度。

传输完成后，接收端保存到预设目录。

发送端和接收端均显示完成提示。

### 4.2 支持内容

必须支持：

- 单文件
- 多文件
- 文件夹
- 大文件
- 多任务队列
- 传输历史记录
- 失败自动重试
- 传输失败提示
- 传输完成提示
- 传输速度显示
- 传输进度显示
- 接收目录配置
- 触发边缘配置

### 4.3 文件夹传输策略

文件夹按目录树逐文件传输。

需要保留原始目录结构。

不要求先压缩成压缩包。

任务进度应按总字节数计算。

目录结构、文件数量、总大小应在传输前完成扫描。

### 4.4 传输队列

传输任务进入队列。

队列状态至少包含：

- 等待中
- 正在传输
- 已完成
- 失败
- 已取消
- 正在重试

队列页面应支持：

- 取消任务
- 重试任务
- 打开接收目录
- 查看失败原因

### 4.5 历史记录

需要历史记录。

历史记录保存：

- 时间
- 方向：发送 / 接收
- 对端设备
- 文件或文件夹名称
- 项目数量
- 总大小
- 状态
- 保存路径
- 失败原因

历史记录页面支持：

- 打开文件位置
- 重新发送
- 清除单条记录
- 清空历史记录

### 4.6 失败重试

传输失败后可自动重试。

重试次数可配置。

默认建议：3 次。

重试间隔可固定，例如 3 秒、10 秒、30 秒。

失败后保留任务记录，允许用户手动重试。

---

## 5. 剪贴板互通功能

### 5.1 基本原则

开启后，两台电脑自动双向同步剪贴板。

无需弹窗确认。

无需手动接收。

无需额外交互。

剪贴板同步应安静运行。

### 5.2 必须支持的类型

第一版必须一次性支持：

- 纯文本
- URL
- 图片剪贴板
- 截图图片剪贴板

URL 按纯文本处理。

图片统一转 PNG 传输。

接收端需要写入系统图片剪贴板。

### 5.3 图片剪贴板

典型场景：

- Windows 截图后自动进入剪贴板，macOS 可直接粘贴。
- macOS 截图复制到剪贴板后，Windows 可直接粘贴。
- 微信截图、系统截图工具、浏览器复制图片等常见图片剪贴板场景。

图片处理策略：

- 发送端读取系统剪贴板图片。
- 统一转换为 PNG。
- 通过局域网发送 PNG 数据。
- 接收端写入本机系统图片剪贴板。

### 5.4 图片限制

需要限制图片体积，避免后台卡顿。

设置项：

- 图片同步开关
- 最大图片体积
- 超过限制后忽略
- 忽略时给轻量提示

默认建议：

- 最大图片体积：20 MB

超过限制时，不同步该图片。

### 5.5 防回环

必须防止剪贴板循环同步。

策略：

- 每次同步内容生成 hash。
- 记录来源设备 ID。
- 记录最近写入时间。
- 本机刚写入的远端内容，再次被系统监听到时直接忽略。

需要避免以下循环：

```text
A 复制 → B 写入 → B 检测变化 → A 写入 → 无限循环
```

### 5.6 明确不做

不做文件列表剪贴板。

不处理：

- Excel 单元格结构
- Word 格式
- Office 私有格式
- 浏览器复杂 HTML 剪贴板
- 富文本
- RTF
- PDF 对象
- 颜色对象
- 音频对象
- 设计软件私有格式
- 任意自定义剪贴板格式

剪贴板边界：

只做文字和图片。

---

## 6. UI 与交互设计

### 6.1 UI 总体要求

应用不能采用工程 UI。

不能做成网页后台风格。

不能把 IP、端口、日志、参数堆在首页。

应用需要消费级软件质感。

首页应当像一个互通控制中心，重点展示：

- 是否已连接
- 对端是谁
- 文件穿越是否可用
- 剪贴板是否开启
- 当前是否有传输任务

### 6.2 UI 层级

应用包含五层 UI：

- 主 UI
- 边缘投递 UI
- 接收 / 传输浮层
- 传输队列与历史记录
- 设置页

### 6.3 主 UI

主 UI 是打开软件后的主要窗口。

首页结构建议：

顶部：

- 应用名
- 连接状态
- 对端设备名
- 设置入口

中间：

- 本机设备 ⇄ 对端设备
- 连接状态
- 文件穿越状态
- 剪贴板互通状态

主要操作：

- 发送文件 / 文件夹
- 边缘投递状态
- 剪贴板开关

底部入口：

- 传输中
- 历史记录
- 设置

首页示意：

```text
┌──────────────────────────────┐
│  Wormhole Link        已连接  │
├──────────────────────────────┤
│                              │
│      这台电脑 ⇄ 对端设备       │
│                              │
│      文件穿越已就绪            │
│      剪贴板互通已开启          │
│                              │
│   [ 发送文件 / 文件夹 ]        │
│   [ 边缘投递：右侧边缘 ]       │
│                              │
├──────────────────────────────┤
│  传输中  0   历史记录  12   设置 │
└──────────────────────────────┘
```

首页不显示日志。

首页不暴露复杂网络参数。

### 6.4 边缘投递 UI

边缘投递 UI 平时隐藏。

用户拖动文件靠近预设边缘时出现。

拖入后显示：

- 目标设备
- 文件数量
- 文件夹数量
- 总大小
- 释放发送提示

示意：

```text
释放发送到 对端设备
3 个项目 · 1.2 GB
```

状态变化：

- 拖入边缘：UI 滑出
- 可接收：高亮
- 拖走：UI 收回
- 松手：加入传输队列

虫洞动画暂不做。

第一版只需要滑出、高亮、收回、进度反馈。

### 6.5 接收 / 传输浮层

对端开始发送后，接收端边缘弹出浮层。

显示：

- 来源设备
- 文件名或文件夹名
- 项目数量
- 总大小
- 进度条
- 速度
- 剩余时间

示意：

```text
来自 ROG 笔记本
正在接收：实验数据文件夹
42% · 86 MB/s · 剩余 18 秒
```

完成后：

```text
接收完成
已保存到 Downloads/Wormhole
[打开文件夹]
```

失败后：

```text
接收失败
网络连接中断
[重试]
```

### 6.6 传输队列页面

队列页面显示当前任务。

分组：

- 正在传输
- 等待中
- 失败
- 已完成

每个任务用卡片展示。

字段：

- 文件名 / 文件夹名
- 方向
- 对端设备
- 大小
- 进度
- 速度
- 状态
- 操作按钮

操作按钮：

- 取消
- 重试
- 打开位置

避免使用密集工程表格。

### 6.7 历史记录页面

历史记录页面用于查看过去的传输。

采用卡片列表。

每条记录显示：

- 时间
- 方向
- 文件名 / 文件夹名
- 大小
- 状态
- 保存位置

支持：

- 打开位置
- 重新发送
- 删除记录

### 6.8 剪贴板状态页面

首页只显示剪贴板开关和状态。

详细页显示：

- 文字同步：开启 / 关闭
- 图片同步：开启 / 关闭
- 图片大小上限
- 最后同步时间
- 最后同步来源
- 当前状态

不显示剪贴板正文。

避免隐私压力。

### 6.9 设置页

设置页分组：

连接设置：

- 对端设备名称
- 对端 IP
- 端口
- 自动连接
- 断线重连
- 测试连接

文件设置：

- 接收目录
- 触发边缘
- 是否允许文件夹
- 失败重试次数
- 历史记录保留天数

剪贴板设置：

- 总开关
- 文字同步
- 图片同步
- 图片大小限制
- 图片统一转 PNG
- 忽略超大剪贴板内容

外观设置：

- 浅色
- 深色
- 跟随系统
- 边缘 UI 透明度
- 系统通知开关

诊断设置：

- 版本号
- 导出日志
- 打开配置目录

---

## 7. 首次启动向导

首次启动需要有引导流程。

流程：

- 欢迎页
- 设置本机名称
- 填写对端 IP 和端口
- 测试连接
- 选择接收目录
- 选择触发边缘
- 开启剪贴板互通
- 进入主界面

欢迎页文案：

```text
连接你的两台电脑
用于局域网内的文件穿越和剪贴板互通
```

连接页：

```text
填写对端电脑的地址
对端 IP：
端口：
[测试连接]
```

权限页：

```text
需要允许访问剪贴板，才能自动同步文字和截图。
[打开系统设置]
```

所有错误提示都应使用用户能看懂的语言。

不要出现开发者日志式提示。

---

## 8. 后台与系统集成

应用需要支持后台常驻。

需要托盘图标。

托盘菜单至少包含：

- 打开主窗口
- 暂停文件穿越
- 暂停剪贴板同步
- 查看连接状态
- 退出

后台资源目标：

- 空闲时只保留连接心跳、剪贴板监听、TCP 监听。
- 不录屏。
- 不持续监听全局鼠标轨迹。
- 不做高频网络扫描。
- 不持续播放动画。
- 边缘 UI 只在拖拽文件接近边缘或传输时显示。

Windows 剪贴板监听优先使用系统事件。

macOS 剪贴板可使用低频轮询，建议 300–800 ms。

---

## 9. 技术实现方案

### 9.1 总体技术路线

本项目采用“核心服务与 UI 解耦”的架构。

核心功能独立为 Core Daemon。

UI 作为可替换外壳，只通过本地 API 与 Core Daemon 通信。

前期可以使用简易 UI 快速验证功能，后期可以彻底重构为消费级 UI，核心传输、剪贴板、队列、历史记录、连接逻辑不需要重写。

推荐总体结构：

```text
UI Shell
主窗口 / 边缘投递 UI / 接收浮层 / 设置页 / 历史记录
        ↓
Local API
HTTP + WebSocket
        ↓
Core Daemon
连接 / 文件传输 / 文件夹扫描 / 队列 / 重试 / 历史 / 剪贴板同步
        ↓
Platform Adapters
Windows 剪贴板 / macOS 剪贴板 / DropZone / 通知 / 托盘 / 开机启动
```

核心原则：

- UI 不直接传输文件。
- UI 不直接监听剪贴板。
- UI 不直接维护队列。
- UI 不直接写历史记录数据库。
- UI 不直接判断连接状态。

UI 只负责展示状态、收集用户操作、承载拖拽交互。

---

## 10. 推荐语言与技术栈

### 10.1 首选方案

推荐采用：

- Core Daemon：Rust
- 桌面 UI：Tauri 2
- 前端 UI：React / Vue / Svelte 三选一
- 本地通信：HTTP + WebSocket
- 数据库：SQLite
- 配置文件：TOML 或 JSON
- 日志：tracing + rolling file appender

首选组合：

```text
Rust Core Daemon + Tauri 2 + React + SQLite
```

理由：

- Rust 适合做后台服务、文件传输、并发任务、跨平台系统适配。
- Tauri 适合做轻量桌面应用壳。
- React/Vue/Svelte 方便后期重构消费级 UI。
- SQLite 适合保存传输历史、任务状态、失败记录。
- HTTP + WebSocket 简单稳定，调试方便，后期也可替换为 gRPC。

### 10.2 备选方案

如果开发者更熟 Flutter，可以采用：

```text
Rust Core Daemon + Flutter UI + HTTP/WebSocket
```

但不建议把核心功能写进 Dart。

Flutter 只负责 UI。

### 10.3 不推荐方案

不推荐：

- Electron 单体应用
- Flutter 单体应用
- Tauri command 里堆满业务逻辑
- 纯前端式状态管理承载核心功能

这些方案前期快，后期 UI 重构时会拖累核心功能。

尤其禁止这种结构：

```text
按钮点击 → UI 扫描文件夹 → UI 创建任务 → UI 写数据库 → UI 发网络请求
```

正确结构：

```text
按钮点击 → UI 调用 Core API
Core 扫描文件夹 → Core 创建任务 → Core 写数据库 → Core 传输文件 → Core 推送事件
UI 根据事件刷新界面
```

---

## 11. 胶水对象与参考项目

### 11.1 LocalSend

定位：

文件传输协议和产品逻辑的第一参考对象。

可参考内容：

- 局域网设备间文件传输流程
- prepare-upload / upload 类型的握手逻辑
- 基于 HTTP/HTTPS 的本地传输思路
- 文件 metadata 设计
- 多文件传输任务结构
- 发送前确认与接收端状态反馈
- 跨平台打包经验

不建议直接照搬内容：

- UI
- 多设备发现
- 手机端逻辑
- 完整应用结构
- 所有协议细节

本项目不做自动发现，因此不需要复制 LocalSend 的广播发现逻辑。

本项目只需要吸收它的本地传输模型：

```text
发送端准备任务
        ↓
接收端确认可接收
        ↓
发送端流式上传
        ↓
接收端写入文件
        ↓
双方更新进度和状态
```

### 11.2 Deskflow / Barrier

定位：

剪贴板共享思路参考对象。

可参考内容：

- 跨设备剪贴板同步思路
- 剪贴板内容变更检测
- 剪贴板同步防回环
- 连接状态管理
- 跨平台系统能力封装思路

不建议直接复制内容：

- 键鼠共享逻辑
- 屏幕边界鼠标穿越逻辑
- 复杂 KVM 架构
- 输入控制相关代码

本项目明确不做键鼠穿越，因此 Deskflow / Barrier 只能作为剪贴板和跨平台工程参考，不能作为主工程底座。

### 11.3 Tauri

定位：

消费级桌面 UI 外壳。

适合承载：

- 主窗口
- 首次启动向导
- 设置页
- 传输队列
- 历史记录
- 托盘入口
- 基础通知入口

不建议让 Tauri command 直接承载复杂业务逻辑。

Tauri 应作为 UI Shell。

真正业务逻辑放进独立 Core Daemon。

### 11.4 系统原生 API

剪贴板和边缘拖拽必须接系统原生能力。

Windows 侧需要关注：

- Win32 Clipboard API
- CF_UNICODETEXT
- CF_DIB / CF_DIBV5
- AddClipboardFormatListener
- WM_CLIPBOARDUPDATE
- COM DropTarget
- 系统通知
- 托盘图标
- 开机启动

macOS 侧需要关注：

- NSPasteboard
- NSPasteboard.changeCount
- public.utf8-plain-text
- public.png
- public.tiff
- NSWindow / NSPanel
- NSDraggingDestination
- UserNotifications
- LaunchAgent / Login Item

图片剪贴板不要依赖普通跨平台剪贴板库解决。

普通库可用于早期文本验证，正式版本需要原生适配层。

---

## 12. 工程架构

### 12.1 推荐仓库结构

推荐使用 monorepo。

```text
wormhole-link/
  crates/
    wormhole-core/
      src/
        connection/
        transfer/
        clipboard/
        queue/
        history/
        config/
        protocol/
        event_bus/
        security/
        error.rs
        lib.rs

    wormhole-daemon/
      src/
        main.rs
        api_http.rs
        api_ws.rs
        lifecycle.rs

    wormhole-platform/
      src/
        lib.rs
        windows/
          clipboard.rs
          dropzone.rs
          notification.rs
          tray.rs
          startup.rs
        macos/
          clipboard.rs
          dropzone.rs
          notification.rs
          tray.rs
          startup.rs

    wormhole-cli/
      src/
        main.rs

  apps/
    desktop-ui/
      src/
        pages/
        components/
        stores/
        api/
        assets/
      src-tauri/

  docs/
    api.md
    protocol.md
    architecture.md

  tests/
    integration/
```

### 12.2 Core Daemon 职责

Core Daemon 负责：

- 设备连接
- 自动连接
- 断线重连
- 心跳检测
- 文件发送
- 文件接收
- 文件夹扫描
- 传输队列
- 失败重试
- 历史记录
- 剪贴板监听
- 剪贴板写入
- 剪贴板防回环
- 配置管理
- 事件推送

Core Daemon 不负责：

- 复杂 UI 展示
- 消费级动画
- 页面路由
- 窗口布局

### 12.3 UI Shell 职责

UI Shell 负责：

- 主窗口
- 设置向导
- 连接状态展示
- 文件选择按钮
- 边缘投递 UI
- 传输浮层
- 队列页面
- 历史记录页面
- 设置页
- 用户通知展示

UI Shell 只通过 API 调用 Core。

UI Shell 不直接持久化业务数据。

### 12.4 Platform Adapter 职责

平台适配层负责把系统能力封装成统一接口。

核心接口包括：

```text
ClipboardPort
DropZonePort
NotificationPort
TrayPort
StartupPort
FileSystemPort
```

Core 只依赖接口，不依赖具体系统 API。

Windows 和 macOS 分别实现这些接口。

后续支持 Windows ↔ Windows 时，只需要复用 Windows Adapter。

---

## 13. 本地 API 设计

### 13.1 API 选择

前期建议使用：

- HTTP：命令请求
- WebSocket：状态事件推送

优点：

- 实现快
- 调试方便
- UI 框架无关
- 后期重构 UI 成本低
- CLI 和测试工具也能直接调用

后期如需强类型接口，可升级为 gRPC。

### 13.2 HTTP API 草案

连接相关：

```text
GET  /local/state
POST /local/connect
POST /local/disconnect
```

文件传输：

```text
POST /local/transfer/send
POST /local/transfer/cancel
POST /local/transfer/retry
GET  /local/transfer/tasks
GET  /local/transfer/history
POST /local/transfer/history/clear
```

剪贴板：

```text
GET  /local/clipboard/status
POST /local/clipboard/enable
POST /local/clipboard/disable
POST /local/clipboard/system/read-send-text
POST /local/clipboard/system/read-send-image
```

设置：

```text
GET  /local/settings
POST /local/settings/update
```

诊断：

```text
GET  /local/events
```

### 13.3 WebSocket 事件草案

Core 向 UI 推送事件：

```text
connection.changed
peer.online
peer.offline

transfer.created
transfer.queued
transfer.started
transfer.progress
transfer.completed
transfer.failed
transfer.retrying
transfer.cancelled

clipboard.synced
clipboard.ignored
clipboard.too_large
clipboard.failed

settings.updated
permission.required
daemon.error
```

UI 不主动轮询进度。

进度由 Core 主动推送。

---

## 14. 数据模型

### 14.1 设备模型

```json
{
  "device_id": "uuid",
  "display_name": "ROG Laptop",
  "platform": "windows",
  "ip": "192.168.1.12",
  "port": 53317,
  "status": "connected",
  "last_seen": 1730000000
}
```

### 14.2 传输任务模型

```json
{
  "task_id": "uuid",
  "direction": "send",
  "peer_device_id": "uuid",
  "root_name": "实验数据",
  "item_count": 42,
  "total_size": 1073741824,
  "transferred_size": 536870912,
  "status": "transferring",
  "speed_bytes_per_sec": 86000000,
  "created_at": 1730000000,
  "updated_at": 1730000012
}
```

### 14.3 文件条目模型

```json
{
  "task_id": "uuid",
  "relative_path": "实验数据/a.txt",
  "size": 12345,
  "sha256": "optional",
  "status": "pending"
}
```

### 14.4 剪贴板载荷模型

文本：

```json
{
  "kind": "text",
  "mime": "text/plain",
  "text": "hello",
  "hash": "sha256",
  "source_device_id": "uuid",
  "created_at": 1730000000
}
```

图片：

```json
{
  "kind": "image",
  "mime": "image/png",
  "size": 1234567,
  "hash": "sha256",
  "source_device_id": "uuid",
  "created_at": 1730000000
}
```

图片二进制不建议直接塞入 JSON。

建议通过单独二进制请求发送。

---

## 15. 数据持久化

### 15.1 SQLite

SQLite 保存：

- 传输历史
- 传输任务
- 失败记录
- 历史路径
- 最近对端状态

推荐表：

```text
devices
transfer_tasks
transfer_items
transfer_history
clipboard_events
settings_snapshot
```

### 15.2 配置文件

配置文件保存用户设置。

建议使用 TOML 或 JSON。

内容包括：

- 对端 IP
- 端口
- 设备名称
- 接收目录
- 触发边缘
- 自动连接
- 断线重连
- 剪贴板开关
- 图片大小限制
- 历史记录保留天数

### 15.3 日志

日志保存到应用数据目录。

日志应支持导出。

日志内容包括：

- 连接状态
- 传输错误
- 重试记录
- 剪贴板错误
- 权限错误
- 系统 API 调用失败

日志不记录剪贴板正文。

日志不记录用户文件内容。

---

## 16. 文件传输实现

### 16.1 基本传输模型

采用本地 HTTP 流式传输。

发送端和接收端都运行本地服务。

每个客户端既可以作为发送端，也可以作为接收端。

传输流程：

```text
UI drop(paths)
        ↓
Core 扫描文件/文件夹
        ↓
生成 manifest
        ↓
创建任务
        ↓
通知对端 prepare-transfer
        ↓
对端确认接收目录可用
        ↓
发送端逐文件上传
        ↓
接收端写入临时文件
        ↓
写入完成后 rename 到正式文件
        ↓
更新任务状态
```

### 16.2 文件夹处理

文件夹必须展开为 manifest。

manifest 包含：

- 相对路径
- 文件大小
- 修改时间
- 可选 hash

传输时按相对路径重建目录结构。

### 16.3 临时文件策略

接收端不要直接写最终文件。

应先写入临时文件：

```text
filename.ext.wormhole_tmp
```

写入完成并校验通过后，再重命名为最终文件。

这样可以避免半截文件污染接收目录。

### 16.4 失败重试策略

第一版建议：

文件级重试。

如果某个文件失败，重试该文件。

小文件从头重试。

大文件后续可升级为分片续传。

推荐默认：

- 自动重试 3 次。
- 重试间隔递增。
- 最终失败后保留任务，允许手动重试。

### 16.5 大文件策略

大文件必须流式读写。

禁止整文件读入内存。

发送端按 stream 读取。

接收端按 stream 写入。

进度按已传输字节计算。

---

## 17. 剪贴板实现

### 17.1 总体策略

剪贴板由 Core Clipboard Service 管理。

UI 不监听剪贴板。

UI 不读取剪贴板正文。

流程：

```text
Platform Adapter 检测剪贴板变化
        ↓
Core 读取通用内容
        ↓
转换为中间格式
        ↓
计算 hash
        ↓
判断是否为远端回写
        ↓
发送到对端
        ↓
对端 Core 接收
        ↓
对端 Platform Adapter 写入系统剪贴板
```

### 17.2 文本剪贴板

Windows 读取 CF_UNICODETEXT。

macOS 读取 string pasteboard type。

统一转换为 UTF-8 text/plain。

### 17.3 图片剪贴板

Windows 优先读取 CF_DIB / CF_DIBV5。

macOS 优先读取 PNG / TIFF。

统一转换为 PNG。

传输 PNG bytes。

接收端根据平台写入系统图片剪贴板。

### 17.4 图片大小限制

Core 在发送前检查 PNG 体积。

超过限制则忽略。

默认上限建议：20 MB。

忽略事件通过 WebSocket 通知 UI。

UI 显示轻量提示。

### 17.5 防回环

Core 保存最近写入的远端剪贴板 hash。

当系统剪贴板再次触发变化时，若 hash 命中最近远端写入记录，则忽略。

记录字段：

- hash
- source_device_id
- written_at
- kind

---

## 18. 边缘投递 UI 实现

### 18.1 设计原则

边缘 UI 属于 UI Shell。

但拖拽完成后的业务逻辑必须交给 Core。

边缘 UI 只负责：

- 检测拖入
- 显示接纳状态
- 获取文件路径
- 调用 Core API
- 显示 Core 返回的任务状态

### 18.2 Windows 实现

Windows 边缘 UI 建议使用：

- 透明置顶窗口
- 无边框窗口
- DropTarget / 拖放接口
- 任务栏不显示
- 失焦后自动隐藏

拖拽文件进入热区后展开 UI。

释放后拿到文件路径数组，调用：

```text
POST /local/transfer/send
```

### 18.3 macOS 实现

macOS 边缘 UI 建议使用：

- NSPanel 或 borderless NSWindow
- 置顶层级
- NSDraggingDestination
- 拖拽进入时展开
- 拖走时收回
- 释放后读取 file URL

释放后调用 Core API。

### 18.4 不做虫洞特效

第一版不做虫洞特效。

第一版只做：

- 滑出
- 高亮
- 收回
- 进入队列提示
- 传输进度浮层

虫洞特效列入后续视觉增强。

---

## 19. 安全与边界

### 19.1 局域网范围

应用只服务局域网。

不做公网穿透。

不做云端中转。

### 19.2 对端固定

第一版只连接一个对端设备。

不做多设备列表。

不做广播发现。

### 19.3 端口暴露

只监听用户配置端口。

建议默认只绑定局域网地址或本机可达网卡。

需要在设置中显示当前监听地址。

### 19.4 简单认证

即使只在局域网使用，也建议做最小认证。

首次手动配置时生成 shared token。

所有 API 请求带 token。

防止局域网内其他设备误调用传输接口。

### 19.5 日志隐私

日志不记录剪贴板正文。

日志不记录文件内容。

日志可以记录文件名、大小、错误码。

---

## 20. 最终工程原则

本项目必须遵循以下原则：

- Core 优先，UI 后置。
- UI 可推翻重做。
- 业务状态归 Core。
- 平台能力走 Adapter。
- 传输协议平台无关。
- 剪贴板统一中间格式。
- 文件传输流式处理。
- 历史记录由 Core 持久化。
- 设置由 Core 统一管理。
- 日志不泄露剪贴板正文。
- 首发 Windows ↔ macOS，架构预留 Windows ↔ Windows。
- 不做键鼠穿越。
- 不做多设备。
- 不做自动发现。
- 不做文件列表剪贴板。
