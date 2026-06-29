use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tao::{
    dpi::LogicalSize,
    event::WindowEvent,
    event_loop::{EventLoopProxy, EventLoopWindowTarget},
    window::{Window, WindowBuilder, WindowId},
};
use wry::{DragDropEvent, WebView, WebViewBuilder};

use crate::local_api_client::LocalApiClient;

pub struct DropZoneWindow {
    webview: WebView,
    window: Window,
}

#[derive(Clone, Debug)]
pub enum DropZoneEvent {
    Enter { paths: Vec<PathBuf> },
    Leave,
    Drop { paths: Vec<PathBuf> },
}

impl DropZoneWindow {
    pub fn new<T>(event_loop: &EventLoopWindowTarget<T>, proxy: EventLoopProxy<T>) -> Result<Self>
    where
        T: From<DropZoneEvent> + Clone + 'static,
    {
        let window = WindowBuilder::new()
            .with_title("Wormhole 原生拖拽投递")
            .with_inner_size(LogicalSize::new(520.0, 360.0))
            .with_min_inner_size(LogicalSize::new(520.0, 360.0))
            .with_resizable(false)
            .with_always_on_top(true)
            .with_visible(false)
            .build(event_loop)?;

        let webview_proxy = proxy.clone();
        let webview = WebViewBuilder::new()
            .with_html(DROPZONE_HTML)
            .with_drag_drop_handler(move |event| {
                match event {
                    DragDropEvent::Enter { paths, .. } => {
                        let _ = webview_proxy.send_event(T::from(DropZoneEvent::Enter { paths }));
                    }
                    DragDropEvent::Drop { paths, .. } => {
                        let _ = webview_proxy.send_event(T::from(DropZoneEvent::Drop { paths }));
                    }
                    DragDropEvent::Leave => {
                        let _ = webview_proxy.send_event(T::from(DropZoneEvent::Leave));
                    }
                    DragDropEvent::Over { .. } => {}
                    _ => {}
                }
                true
            })
            .build(&window)?;

        window.set_visible(true);
        window.set_focus();

        let drop_zone = Self { webview, window };
        drop_zone.set_idle()?;
        Ok(drop_zone)
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn show(&self) {
        self.window.set_visible(true);
        self.window.set_focus();
        let _ = self.set_idle();
    }

    pub fn handle_window_event(&self, event: &WindowEvent) -> DropZoneAction {
        match event {
            WindowEvent::CloseRequested => DropZoneAction::Close,
            _ => DropZoneAction::Keep,
        }
    }

    pub fn handle_drop_event(&self, event: DropZoneEvent, client: &LocalApiClient) {
        match event {
            DropZoneEvent::Enter { paths } => {
                let _ = self.set_hovering(&paths);
            }
            DropZoneEvent::Leave => {
                let _ = self.set_idle();
            }
            DropZoneEvent::Drop { paths } => {
                if paths.is_empty() {
                    let _ = self.set_error("没有收到可发送的文件路径");
                    return;
                }

                let label = paths_label(&paths);
                let _ = self.set_sending(&label);

                if !peer_is_connected(client) {
                    let _ = self.set_offline(&label);
                    return;
                }

                match client.send_paths(&paths) {
                    Ok(()) => {
                        let _ = self.set_queued(&label);
                    }
                    Err(err) => {
                        let _ = self.set_error(&format!("投递失败：{err}"));
                    }
                }
            }
        }
    }

    fn set_idle(&self) -> Result<()> {
        self.render(ViewState {
            phase: "idle",
            eyebrow: "原生拖拽投递",
            title: "拖入文件或文件夹",
            message: "释放后发送到已连接的对端设备。",
            detail: "等待真实系统拖拽事件",
            file_label: "",
        })
    }

    fn set_hovering(&self, paths: &[PathBuf]) -> Result<()> {
        let label = paths_label(paths);
        self.render(ViewState {
            phase: "ready",
            eyebrow: "准备发送",
            title: "松开即可加入队列",
            message: "Wormhole 已拿到系统提供的真实路径。",
            detail: "文件会按当前传输设置并行发送",
            file_label: &label,
        })
    }

    fn set_sending(&self, label: &str) -> Result<()> {
        self.render(ViewState {
            phase: "sending",
            eyebrow: "正在提交",
            title: "正在加入传输队列",
            message: "请稍候，daemon 正在接收发送请求。",
            detail: "不会伪造完成状态",
            file_label: label,
        })
    }

    fn set_queued(&self, label: &str) -> Result<()> {
        self.render(ViewState {
            phase: "queued",
            eyebrow: "已接收",
            title: "已加入传输队列",
            message: "进度会在控制中心的传输页实时更新。",
            detail: "完成状态来自后端真实任务",
            file_label: label,
        })
    }

    fn set_offline(&self, label: &str) -> Result<()> {
        self.render(ViewState {
            phase: "offline",
            eyebrow: "对端离线",
            title: "暂时不能投递",
            message: "重新连接后再释放文件，离线状态不会假装成功。",
            detail: "从菜单栏或托盘选择重新连接",
            file_label: label,
        })
    }

    fn set_error(&self, message: &str) -> Result<()> {
        self.render(ViewState {
            phase: "error",
            eyebrow: "投递失败",
            title: "没有加入队列",
            message,
            detail: "请查看控制中心诊断页或日志",
            file_label: "",
        })
    }

    fn render(&self, state: ViewState<'_>) -> Result<()> {
        self.window.set_title(state.title);
        let payload = serde_json::to_string(&state)?;
        self.webview.evaluate_script(&format!(
            "window.wormholeDropState && window.wormholeDropState({payload});"
        ))?;
        Ok(())
    }
}

pub enum DropZoneAction {
    Keep,
    Close,
}

#[derive(Serialize)]
struct ViewState<'a> {
    phase: &'a str,
    eyebrow: &'a str,
    title: &'a str,
    message: &'a str,
    detail: &'a str,
    file_label: &'a str,
}

fn peer_is_connected(client: &LocalApiClient) -> bool {
    client
        .state()
        .map(|state| state.status == "connected")
        .unwrap_or(false)
}

fn paths_label(paths: &[PathBuf]) -> String {
    match paths {
        [] => "未选择文件".to_string(),
        [path] => compact_path(path),
        [first, rest @ ..] => format!("{} 等 {} 项", compact_path(first), rest.len() + 1),
    }
}

fn compact_path(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

const DROPZONE_HTML: &str = r###"
<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<style>
:root {
  color-scheme: dark;
  --accent: #21d17d;
  --accent-soft: rgba(33, 209, 125, 0.20);
  --ink: #f5f7fa;
  --muted: #a8b2c1;
  --panel: #10151d;
  --panel-2: #151c25;
  --line: rgba(255, 255, 255, 0.12);
}

* { box-sizing: border-box; }

html,
body {
  width: 100%;
  height: 100%;
  margin: 0;
  overflow: hidden;
  background:
    radial-gradient(circle at 28% 12%, rgba(33, 209, 125, 0.12), transparent 32%),
    linear-gradient(145deg, #0c1016, #171f2a 58%, #0b1016);
  color: var(--ink);
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Microsoft YaHei", sans-serif;
  letter-spacing: 0;
}

.surface {
  position: relative;
  display: grid;
  grid-template-rows: auto 1fr auto;
  min-height: 100%;
  padding: 28px 30px 24px;
}

.surface::before {
  content: "";
  position: absolute;
  inset: 14px;
  border: 1px solid var(--line);
  border-radius: 22px;
  pointer-events: none;
}

.top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 28px;
}

.brand {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 13px;
  font-weight: 700;
  color: rgba(245, 247, 250, 0.88);
}

.mark {
  width: 24px;
  height: 24px;
  border-radius: 8px;
  background:
    linear-gradient(135deg, rgba(33, 209, 125, 0.95), rgba(56, 147, 255, 0.92));
  box-shadow: 0 0 24px rgba(33, 209, 125, 0.22);
}

.chip {
  min-width: 92px;
  padding: 6px 10px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 999px;
  color: var(--muted);
  font-size: 12px;
  text-align: center;
  transition: border-color 180ms ease, color 180ms ease, background 180ms ease;
}

.stage {
  position: relative;
  display: grid;
  place-items: center;
  min-height: 230px;
}

.target {
  position: relative;
  width: 176px;
  height: 176px;
  display: grid;
  place-items: center;
  border: 1px solid rgba(255, 255, 255, 0.15);
  border-radius: 999px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.08), rgba(255, 255, 255, 0.02)),
    var(--panel);
  box-shadow:
    0 22px 80px rgba(0, 0, 0, 0.35),
    inset 0 1px 0 rgba(255, 255, 255, 0.10);
  transform: scale(1);
  transition: transform 220ms ease, border-color 220ms ease, box-shadow 220ms ease;
}

.target::before,
.target::after {
  content: "";
  position: absolute;
  inset: 13px;
  border-radius: 999px;
  border: 1px solid var(--accent-soft);
  animation: breathe 2.7s ease-in-out infinite;
}

.target::after {
  inset: -9px;
  opacity: 0.34;
  animation-delay: 700ms;
}

.glyph {
  width: 74px;
  height: 74px;
  border-radius: 25px;
  display: grid;
  place-items: center;
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.12);
  transition: background 180ms ease, transform 180ms ease;
}

.arrow {
  width: 26px;
  height: 34px;
  position: relative;
}

.arrow::before,
.arrow::after {
  content: "";
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
  background: var(--accent);
}

.arrow::before {
  top: 0;
  width: 7px;
  height: 29px;
  border-radius: 999px;
}

.arrow::after {
  bottom: 0;
  width: 24px;
  height: 24px;
  clip-path: polygon(50% 100%, 0 36%, 28% 36%, 28% 0, 72% 0, 72% 36%, 100% 36%);
}

.copy {
  position: absolute;
  bottom: 0;
  width: min(420px, 100%);
  text-align: center;
}

.eyebrow {
  margin-bottom: 8px;
  color: var(--accent);
  font-size: 12px;
  font-weight: 800;
}

.title {
  margin: 0;
  color: var(--ink);
  font-size: 25px;
  line-height: 1.16;
  font-weight: 820;
}

.message {
  margin: 10px auto 0;
  max-width: 390px;
  color: var(--muted);
  font-size: 14px;
  line-height: 1.45;
}

.file {
  margin: 14px auto 0;
  max-width: 392px;
  min-height: 28px;
  padding: 7px 12px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(245, 247, 250, 0.88);
  font-size: 13px;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  opacity: 0;
  transform: translateY(5px);
  transition: opacity 180ms ease, transform 180ms ease;
}

.foot {
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: rgba(168, 178, 193, 0.72);
  font-size: 12px;
}

.pulse {
  width: 8px;
  height: 8px;
  border-radius: 999px;
  background: var(--accent);
  box-shadow: 0 0 18px var(--accent);
  animation: dot 1.55s ease-in-out infinite;
}

body.ready .target,
body.sending .target {
  transform: scale(1.055);
  border-color: rgba(33, 209, 125, 0.62);
  box-shadow:
    0 28px 90px rgba(0, 0, 0, 0.42),
    0 0 56px rgba(33, 209, 125, 0.18),
    inset 0 1px 0 rgba(255, 255, 255, 0.13);
}

body.ready .glyph,
body.sending .glyph {
  background: rgba(33, 209, 125, 0.13);
  transform: translateY(-3px);
}

body.ready .file,
body.sending .file,
body.queued .file,
body.offline .file {
  opacity: 1;
  transform: translateY(0);
}

body.queued {
  --accent: #21d17d;
  --accent-soft: rgba(33, 209, 125, 0.24);
}

body.queued .target {
  animation: success 620ms cubic-bezier(.2, .9, .2, 1) 1;
}

body.offline,
body.error {
  --accent: #ffb64a;
  --accent-soft: rgba(255, 182, 74, 0.22);
}

body.error {
  --accent: #ff5c7a;
  --accent-soft: rgba(255, 92, 122, 0.22);
}

body.ready .chip,
body.sending .chip,
body.queued .chip {
  color: #0c1510;
  border-color: transparent;
  background: var(--accent);
}

body.offline .chip,
body.error .chip {
  color: #16110a;
  border-color: transparent;
  background: var(--accent);
}

@keyframes breathe {
  0%, 100% { transform: scale(0.96); opacity: 0.38; }
  50% { transform: scale(1.07); opacity: 0.86; }
}

@keyframes dot {
  0%, 100% { transform: scale(0.72); opacity: 0.54; }
  50% { transform: scale(1); opacity: 1; }
}

@keyframes success {
  0% { transform: scale(1.04); }
  45% { transform: scale(1.12); }
  100% { transform: scale(1); }
}
</style>
</head>
<body class="idle">
  <main class="surface">
    <header class="top">
      <div class="brand"><span class="mark"></span><span>Wormhole</span></div>
      <div class="chip" id="chip">待命</div>
    </header>
    <section class="stage">
      <div class="target" aria-hidden="true">
        <div class="glyph"><div class="arrow"></div></div>
      </div>
      <div class="copy">
        <div class="eyebrow" id="eyebrow">原生拖拽投递</div>
        <h1 class="title" id="title">拖入文件或文件夹</h1>
        <p class="message" id="message">释放后发送到已连接的对端设备。</p>
        <div class="file" id="file"></div>
      </div>
    </section>
    <footer class="foot">
      <span id="detail">等待真实系统拖拽事件</span>
      <span class="pulse"></span>
    </footer>
  </main>
<script>
const phaseChip = {
  idle: "待命",
  ready: "可释放",
  sending: "提交中",
  queued: "已入队",
  offline: "离线",
  error: "失败"
};

window.addEventListener("dragover", (event) => event.preventDefault());
window.addEventListener("drop", (event) => event.preventDefault());

window.wormholeDropState = (state) => {
  const phase = state.phase || "idle";
  document.body.className = phase;
  document.getElementById("chip").textContent = phaseChip[phase] || phase;
  document.getElementById("eyebrow").textContent = state.eyebrow || "";
  document.getElementById("title").textContent = state.title || "";
  document.getElementById("message").textContent = state.message || "";
  document.getElementById("detail").textContent = state.detail || "";
  document.getElementById("file").textContent = state.file_label || "";
};
</script>
</body>
</html>
"###;
