mod app_lifecycle;
mod browser_open;
mod daemon_manager;
mod local_api_client;
mod native_file_picker;
mod tray_controller;

fn main() -> anyhow::Result<()> {
    app_lifecycle::run()
}
