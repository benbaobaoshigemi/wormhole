use anyhow::Result;
use std::time::{Duration, Instant};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tray_icon::{
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIconBuilder,
};

use crate::{
    browser_open,
    daemon_manager::DaemonManager,
    dropzone_controller::{DropZoneAction, DropZoneEvent, DropZoneWindow},
    native_file_picker::{pick_files, pick_folder},
    startup_controller,
};

#[derive(Clone, Debug)]
enum TrayEvent {
    DropZone(DropZoneEvent),
}

impl From<DropZoneEvent> for TrayEvent {
    fn from(value: DropZoneEvent) -> Self {
        Self::DropZone(value)
    }
}

pub fn run_tray(mut daemon: DaemonManager) -> Result<()> {
    let event_loop = EventLoopBuilder::<TrayEvent>::with_user_event().build();
    let event_proxy = event_loop.create_proxy();
    let menu = Menu::new();
    let status_item = MenuItem::new("状态：读取中", false, None);
    let peer_item = MenuItem::new("对端：-", false, None);
    let open_center = MenuItem::new("打开控制中心", true, None);
    let edge_drop_toggle = CheckMenuItem::new("启用边缘投递", true, true, None);
    let send_file = MenuItem::new("发送文件", true, None);
    let send_folder = MenuItem::new("发送文件夹", true, None);
    let open_receive = MenuItem::new("打开接收目录", true, None);
    let clipboard_toggle = CheckMenuItem::new("剪贴板同步", true, false, None);
    let startup_toggle = CheckMenuItem::new(
        "开机启动 Wormhole",
        true,
        startup_controller::is_enabled().unwrap_or(false),
        None,
    );
    let restart = MenuItem::new("重新连接 / 重启 daemon", true, None);
    let open_logs = MenuItem::new("打开日志目录", true, None);
    let about = MenuItem::new("关于 Wormhole 0.1.0", false, None);
    let quit = MenuItem::new("退出 Wormhole", true, None);

    menu.append_items(&[
        &status_item,
        &peer_item,
        &PredefinedMenuItem::separator(),
        &open_center,
        &edge_drop_toggle,
        &send_file,
        &send_folder,
        &open_receive,
        &PredefinedMenuItem::separator(),
        &clipboard_toggle,
        &startup_toggle,
        &restart,
        &open_logs,
        &about,
        &PredefinedMenuItem::separator(),
        &quit,
    ])?;

    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
    let mut tray_builder = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Wormhole")
        .with_icon(wormhole_icon()?);
    #[cfg(target_os = "macos")]
    {
        tray_builder = tray_builder.with_icon_as_template(true);
    }
    let _tray = tray_builder.build()?;

    let menu_channel = MenuEvent::receiver();
    let mut last_state_refresh = Instant::now() - Duration::from_secs(60);
    let mut drop_window: Option<DropZoneWindow> = None;
    let mut edge_drop_enabled = true;

    let client = daemon.client();
    let control_center_url = daemon.control_center_url();
    browser_open::open_control_center(&control_center_url).ok();

    event_loop.run(move |event, event_loop_target, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(250));

        if matches!(
            event,
            Event::NewEvents(StartCause::ResumeTimeReached { .. } | StartCause::Init)
        ) {
            if edge_drop_enabled && drop_window.is_none() {
                match DropZoneWindow::new(event_loop_target, event_proxy.clone()) {
                    Ok(window) => drop_window = Some(window),
                    Err(err) => {
                        eprintln!("EdgeDropZone failed to start: {err:?}");
                        status_item.set_text(format!("边缘投递不可用：{err}"));
                        edge_drop_enabled = false;
                        edge_drop_toggle.set_checked(false);
                    }
                }
            }
            if let Some(window) = &drop_window {
                window.tick();
            }
            if last_state_refresh.elapsed() > Duration::from_secs(3) {
                if let Ok(state) = client.state() {
                    status_item.set_text(format!("状态：{}", state.status));
                    peer_item.set_text(format!(
                        "对端：{}",
                        state
                            .peer
                            .map(|peer| peer.device_name)
                            .unwrap_or_else(|| "未连接".to_string())
                    ));
                    clipboard_toggle.set_checked(state.settings.clipboard.enabled);
                } else if daemon.restart().is_ok() {
                    status_item.set_text("状态：daemon 已重启");
                } else {
                    status_item.set_text("状态：daemon 不可用");
                }
                last_state_refresh = Instant::now();
            }
        }

        if let Event::WindowEvent {
            event, window_id, ..
        } = &event
        {
            if drop_window
                .as_ref()
                .map(|window| window.id() == *window_id)
                .unwrap_or(false)
            {
                if matches!(
                    drop_window
                        .as_ref()
                        .map(|window| window.handle_window_event(event)),
                    Some(DropZoneAction::Close)
                ) {
                    drop_window = None;
                    edge_drop_enabled = false;
                    edge_drop_toggle.set_checked(false);
                }
            }
        }

        if let Event::UserEvent(TrayEvent::DropZone(event)) = &event {
            if let Some(window) = &drop_window {
                window.handle_drop_event(event.clone(), &client);
            }
        }

        while let Ok(event) = menu_channel.try_recv() {
            let id = event.id;
            if id == open_center.id() {
                let _ = browser_open::open_control_center(&control_center_url);
            } else if id == edge_drop_toggle.id() {
                edge_drop_enabled = !edge_drop_enabled;
                edge_drop_toggle.set_checked(edge_drop_enabled);
                if edge_drop_enabled {
                    match DropZoneWindow::new(event_loop_target, event_proxy.clone()) {
                        Ok(window) => drop_window = Some(window),
                        Err(err) => {
                            eprintln!("EdgeDropZone failed to start: {err:?}");
                            status_item.set_text(format!("边缘投递不可用：{err}"));
                            edge_drop_enabled = false;
                            edge_drop_toggle.set_checked(false);
                        }
                    }
                } else if let Some(window) = &drop_window {
                    window.disable();
                    drop_window = None;
                }
            } else if id == send_file.id() {
                if let Some(paths) = pick_files() {
                    let _ = client.send_paths(&paths);
                }
            } else if id == send_folder.id() {
                if let Some(paths) = pick_folder() {
                    let _ = client.send_paths(&paths);
                }
            } else if id == open_receive.id() {
                let _ = client.open_receive_dir();
            } else if id == clipboard_toggle.id() {
                if clipboard_toggle.is_checked() {
                    let _ = client.disable_clipboard();
                    clipboard_toggle.set_checked(false);
                } else {
                    let _ = client.enable_clipboard();
                    clipboard_toggle.set_checked(true);
                }
            } else if id == startup_toggle.id() {
                let next = !startup_toggle.is_checked();
                if startup_controller::set_enabled(next).is_ok() {
                    startup_toggle.set_checked(next);
                }
            } else if id == restart.id() {
                let _ = daemon.restart();
            } else if id == open_logs.id() {
                let _ = browser_open::open_path(daemon.log_dir());
            } else if id == quit.id() {
                daemon.quit_all();
                *control_flow = ControlFlow::Exit;
            }
        }
    });
}

fn wormhole_icon() -> Result<Icon> {
    #[cfg(target_os = "macos")]
    let bytes = include_bytes!("../../../assets/wormhole/wormhole-tray-template.png").as_slice();
    #[cfg(not(target_os = "macos"))]
    let bytes = include_bytes!("../../../assets/wormhole/wormhole-tray.png").as_slice();

    let image = image::load_from_memory(bytes)?.into_rgba8();
    let (width, height) = image.dimensions();
    Ok(Icon::from_rgba(image.into_raw(), width, height)?)
}
