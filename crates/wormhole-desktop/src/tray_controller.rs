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
    native_file_picker::{pick_files, pick_folder},
};

pub fn run_tray(mut daemon: DaemonManager) -> Result<()> {
    let event_loop = EventLoopBuilder::new().build();
    let menu = Menu::new();
    let status_item = MenuItem::new("状态：读取中", false, None);
    let peer_item = MenuItem::new("对端：-", false, None);
    let open_center = MenuItem::new("打开控制中心", true, None);
    let send_file = MenuItem::new("发送文件", true, None);
    let send_folder = MenuItem::new("发送文件夹", true, None);
    let open_receive = MenuItem::new("打开接收目录", true, None);
    let clipboard_toggle = CheckMenuItem::new("剪贴板同步", true, false, None);
    let restart = MenuItem::new("重新连接 / 重启 daemon", true, None);
    let open_logs = MenuItem::new("打开日志目录", true, None);
    let about = MenuItem::new("关于 Wormhole 0.1.0", false, None);
    let quit = MenuItem::new("退出 Wormhole", true, None);

    menu.append_items(&[
        &status_item,
        &peer_item,
        &PredefinedMenuItem::separator(),
        &open_center,
        &send_file,
        &send_folder,
        &open_receive,
        &PredefinedMenuItem::separator(),
        &clipboard_toggle,
        &restart,
        &open_logs,
        &about,
        &PredefinedMenuItem::separator(),
        &quit,
    ])?;

    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Wormhole")
        .with_icon(wormhole_icon()?)
        .build()?;

    let menu_channel = MenuEvent::receiver();
    let mut last_state_refresh = Instant::now() - Duration::from_secs(60);

    let client = daemon.client();
    let control_center_url = daemon.control_center_url();
    browser_open::open_control_center(&control_center_url).ok();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(250));

        if matches!(
            event,
            Event::NewEvents(StartCause::ResumeTimeReached { .. } | StartCause::Init)
        ) {
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

        while let Ok(event) = menu_channel.try_recv() {
            let id = event.id;
            if id == open_center.id() {
                let _ = browser_open::open_control_center(&control_center_url);
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
    let mut rgba = Vec::with_capacity(16 * 16 * 4);
    for y in 0..16 {
        for x in 0..16 {
            let dx = x as i32 - 8;
            let dy = y as i32 - 8;
            let inside = dx * dx + dy * dy <= 49;
            if inside {
                rgba.extend_from_slice(&[20, 61, 107, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Ok(Icon::from_rgba(rgba, 16, 16)?)
}
