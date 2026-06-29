mod app_lifecycle;
mod browser_open;
mod daemon_manager;
mod dropzone_controller;
mod local_api_client;
mod native_file_picker;
mod startup_controller;
mod tray_controller;

fn main() -> anyhow::Result<()> {
    app_lifecycle::run()
}
