use anyhow::Result;

use crate::{daemon_manager::DaemonManager, tray_controller};

pub fn run() -> Result<()> {
    let daemon = DaemonManager::start_or_attach()?;
    tray_controller::run_tray(daemon)
}
