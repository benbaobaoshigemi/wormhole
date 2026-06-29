use anyhow::{anyhow, Result};
use serde::Serialize;
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tao::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::WindowEvent,
    event_loop::{EventLoopProxy, EventLoopWindowTarget},
    window::{Window, WindowBuilder, WindowId},
};
use wry::{DragDropEvent, WebView, WebViewBuilder};

use crate::local_api_client::LocalApiClient;

const EDGE_MARGIN: i32 = 28;
const IDLE_WIDTH: u32 = 6;
const ACTIVE_WIDTH: u32 = 360;
const MIN_HEIGHT: u32 = 460;

pub struct DropZoneWindow {
    active_until: Cell<Option<Instant>>,
    geometry: EdgeGeometry,
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
        let monitor = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
            .ok_or_else(|| anyhow!("no monitor available for edge drop zone"))?;
        let geometry = EdgeGeometry::from_monitor(monitor.position(), monitor.size());

        let window = WindowBuilder::new()
            .with_title("Wormhole 边缘投递")
            .with_inner_size(geometry.idle_size())
            .with_position(geometry.idle_position())
            .with_resizable(false)
            .with_decorations(false)
            .with_always_on_top(true)
            .with_visible(false)
            .build(event_loop)?;

        let webview_proxy = proxy.clone();
        let webview = WebViewBuilder::new()
            .with_html(EDGE_DROPZONE_HTML)
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

        let drop_zone = Self {
            active_until: Cell::new(None),
            geometry,
            webview,
            window,
        };
        drop_zone.collapse()?;
        drop_zone.window.set_visible(true);
        Ok(drop_zone)
    }

    pub fn id(&self) -> WindowId {
        self.window.id()
    }

    pub fn disable(&self) {
        self.window.set_visible(false);
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
                let _ = self.expand();
                let _ = self.set_ready(&paths, client);
            }
            DropZoneEvent::Leave => {
                let _ = self.collapse();
            }
            DropZoneEvent::Drop { paths } => {
                if paths.is_empty() {
                    let _ =
                        self.show_feedback("error", "没有可发送的路径", "系统没有提供文件路径", "");
                    return;
                }

                let label = paths_label(&paths);
                let _ =
                    self.show_feedback("sending", "正在加入队列", "正在提交真实文件路径", &label);

                if !peer_is_connected(client) {
                    let _ =
                        self.show_feedback("offline", "对端离线", "重新连接后再拖到边缘", &label);
                    self.active_until
                        .set(Some(Instant::now() + Duration::from_secs(3)));
                    return;
                }

                match client.send_paths(&paths) {
                    Ok(()) => {
                        let _ = self.show_feedback(
                            "queued",
                            "已加入传输队列",
                            "到控制中心查看真实进度",
                            &label,
                        );
                    }
                    Err(err) => {
                        let _ = self.show_feedback("error", "投递失败", &err.to_string(), &label);
                    }
                }
                self.active_until
                    .set(Some(Instant::now() + Duration::from_secs(2)));
            }
        }
    }

    pub fn tick(&self) {
        if self
            .active_until
            .get()
            .map(|deadline| Instant::now() >= deadline)
            .unwrap_or(false)
        {
            let _ = self.collapse();
        }
    }

    fn collapse(&self) -> Result<()> {
        self.active_until.set(None);
        self.window.set_inner_size(self.geometry.idle_size());
        self.window
            .set_outer_position(self.geometry.idle_position());
        self.render(ViewState {
            phase: "idle",
            eyebrow: "EdgeDropZone",
            title: "边缘投递",
            message: "拖文件到屏幕右侧边缘",
            file_label: "",
        })
    }

    fn expand(&self) -> Result<()> {
        self.window.set_inner_size(self.geometry.active_size());
        self.window
            .set_outer_position(self.geometry.active_position());
        Ok(())
    }

    fn set_ready(&self, paths: &[PathBuf], client: &LocalApiClient) -> Result<()> {
        let label = paths_label(paths);
        let (phase, title, message) = if peer_is_connected(client) {
            ("ready", "松开发送", "发送到已连接的对端电脑")
        } else {
            ("offline", "对端离线", "当前不能投递，先重新连接")
        };
        self.render(ViewState {
            phase,
            eyebrow: "Wormhole 边缘投递",
            title,
            message,
            file_label: &label,
        })
    }

    fn show_feedback(
        &self,
        phase: &'static str,
        title: &str,
        message: &str,
        file_label: &str,
    ) -> Result<()> {
        self.expand()?;
        self.render(ViewState {
            phase,
            eyebrow: "Wormhole 边缘投递",
            title,
            message,
            file_label,
        })
    }

    fn render(&self, state: ViewState<'_>) -> Result<()> {
        self.window.set_title(state.title);
        let payload = serde_json::to_string(&state)?;
        self.webview.evaluate_script(&format!(
            "window.wormholeEdgeState && window.wormholeEdgeState({payload});"
        ))?;
        Ok(())
    }
}

pub enum DropZoneAction {
    Keep,
    Close,
}

#[derive(Clone, Copy)]
struct EdgeGeometry {
    active_width: u32,
    height: u32,
    idle_width: u32,
    left_active: i32,
    left_idle: i32,
    top: i32,
}

impl EdgeGeometry {
    fn from_monitor(position: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> Self {
        let usable_height = size.height.saturating_sub((EDGE_MARGIN * 2) as u32);
        let height = usable_height.clamp(MIN_HEIGHT, size.height.max(MIN_HEIGHT));
        let top = position.y + ((size.height.saturating_sub(height)) / 2) as i32;
        let right = position.x + size.width as i32;
        Self {
            active_width: ACTIVE_WIDTH,
            height,
            idle_width: IDLE_WIDTH,
            left_active: right - ACTIVE_WIDTH as i32,
            left_idle: right - IDLE_WIDTH as i32,
            top,
        }
    }

    fn active_position(&self) -> PhysicalPosition<i32> {
        PhysicalPosition::new(self.left_active, self.top)
    }

    fn active_size(&self) -> PhysicalSize<u32> {
        PhysicalSize::new(self.active_width, self.height)
    }

    fn idle_position(&self) -> PhysicalPosition<i32> {
        PhysicalPosition::new(self.left_idle, self.top)
    }

    fn idle_size(&self) -> PhysicalSize<u32> {
        PhysicalSize::new(self.idle_width, self.height)
    }
}

#[derive(Serialize)]
struct ViewState<'a> {
    phase: &'a str,
    eyebrow: &'a str,
    title: &'a str,
    message: &'a str,
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
        [] => String::new(),
        [path] => compact_path(path),
        [first, rest @ ..] => format!("{} 等 {} 项", compact_path(first), rest.len() + 1),
    }
}

fn compact_path(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

const EDGE_DROPZONE_HTML: &str = r###"
<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<style>
:root {
  color-scheme: dark;
  --accent: #27d17f;
  --accent-soft: rgba(39, 209, 127, 0.24);
  --bad: #ffb454;
  --error: #ff5f7e;
  --ink: rgba(248, 250, 252, 0.96);
  --muted: rgba(203, 213, 225, 0.72);
  --glass: rgba(10, 14, 21, 0.78);
}

* { box-sizing: border-box; }

html,
body {
  width: 100%;
  height: 100%;
  margin: 0;
  overflow: hidden;
  background: transparent;
  color: var(--ink);
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", "Microsoft YaHei", sans-serif;
  letter-spacing: 0;
  user-select: none;
}

body {
  display: flex;
  justify-content: flex-end;
  background: transparent;
}

.edge {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  padding: 12px 0 12px 12px;
}

.rail {
  position: absolute;
  top: 14px;
  right: 0;
  bottom: 14px;
  width: 0;
  border-radius: 999px 0 0 999px;
  background: transparent;
  box-shadow: none;
  opacity: 0;
  transition: width 160ms ease, opacity 160ms ease, box-shadow 160ms ease;
}

.panel {
  width: 318px;
  min-height: 264px;
  margin-right: 14px;
  padding: 22px;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 24px 0 0 24px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.08), rgba(255, 255, 255, 0.025)),
    var(--glass);
  box-shadow: -24px 0 80px rgba(0, 0, 0, 0.36), 0 0 50px rgba(39, 209, 127, 0.12);
  backdrop-filter: blur(22px);
  transform: translateX(330px) scale(0.98);
  opacity: 0;
  transition: transform 220ms cubic-bezier(.2,.8,.2,1), opacity 160ms ease;
}

.glyph {
  width: 62px;
  height: 62px;
  display: grid;
  place-items: center;
  border-radius: 18px;
  background: rgba(39, 209, 127, 0.13);
  border: 1px solid rgba(39, 209, 127, 0.24);
  box-shadow: 0 0 30px rgba(39, 209, 127, 0.16);
}

.arrow {
  width: 34px;
  height: 16px;
  position: relative;
}

.arrow::before,
.arrow::after {
  content: "";
  position: absolute;
  top: 50%;
  background: var(--accent);
}

.arrow::before {
  right: 0;
  width: 32px;
  height: 6px;
  border-radius: 999px;
  transform: translateY(-50%);
}

.arrow::after {
  right: 0;
  width: 18px;
  height: 18px;
  transform: translateY(-50%) rotate(45deg);
  border-radius: 3px 3px 3px 0;
  clip-path: polygon(100% 0, 100% 100%, 0 100%);
}

.eyebrow {
  margin-top: 22px;
  color: var(--accent);
  font-size: 12px;
  font-weight: 800;
}

h1 {
  margin: 8px 0 0;
  font-size: 30px;
  line-height: 1.04;
  font-weight: 840;
}

.message {
  margin: 12px 0 0;
  color: var(--muted);
  font-size: 14px;
  line-height: 1.45;
}

.file {
  margin-top: 20px;
  min-height: 34px;
  padding: 8px 10px;
  border: 1px solid rgba(255, 255, 255, 0.10);
  border-radius: 11px;
  background: rgba(255, 255, 255, 0.07);
  color: rgba(248, 250, 252, 0.88);
  font-size: 13px;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.hint {
  margin-top: 20px;
  display: flex;
  align-items: center;
  gap: 8px;
  color: rgba(203, 213, 225, 0.62);
  font-size: 12px;
}

.dot {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: var(--accent);
  box-shadow: 0 0 18px var(--accent);
}

body.ready .panel,
body.sending .panel,
body.queued .panel,
body.offline .panel,
body.error .panel {
  opacity: 1;
  transform: translateX(0) scale(1);
}

body.ready .rail,
body.sending .rail,
body.queued .rail,
body.offline .rail,
body.error .rail {
  width: 18px;
  opacity: 1;
  background: linear-gradient(180deg, rgba(39, 209, 127, 0.20), var(--accent), rgba(39, 209, 127, 0.20));
  box-shadow: 0 0 34px rgba(39, 209, 127, 0.44);
}

body.queued .panel {
  animation: queuedPulse 620ms cubic-bezier(.2,.8,.2,1) 1;
}

body.offline {
  --accent: var(--bad);
}

body.error {
  --accent: var(--error);
}

body.offline .glyph,
body.error .glyph {
  background: rgba(255, 180, 84, 0.13);
  border-color: rgba(255, 180, 84, 0.24);
}

body.error .glyph {
  background: rgba(255, 95, 126, 0.13);
  border-color: rgba(255, 95, 126, 0.24);
}

@keyframes queuedPulse {
  0% { transform: translateX(0) scale(1); }
  42% { transform: translateX(-10px) scale(1.015); }
  100% { transform: translateX(0) scale(1); }
}
</style>
</head>
<body class="idle">
  <main class="edge">
    <div class="rail"></div>
    <section class="panel">
      <div class="glyph"><div class="arrow"></div></div>
      <div class="eyebrow" id="eyebrow">EdgeDropZone</div>
      <h1 id="title">边缘投递</h1>
      <p class="message" id="message">拖文件到屏幕右侧边缘</p>
      <div class="file" id="file"></div>
      <div class="hint"><span class="dot"></span><span>释放动作来自系统拖拽事件</span></div>
    </section>
  </main>
<script>
window.addEventListener("dragover", event => event.preventDefault());
window.addEventListener("drop", event => event.preventDefault());

window.wormholeEdgeState = state => {
  const phase = state.phase || "idle";
  document.body.className = phase;
  document.getElementById("eyebrow").textContent = state.eyebrow || "";
  document.getElementById("title").textContent = state.title || "";
  document.getElementById("message").textContent = state.message || "";
  document.getElementById("file").textContent = state.file_label || "";
};
</script>
</body>
</html>
"###;
