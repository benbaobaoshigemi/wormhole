#[cfg(target_os = "macos")]
use anyhow::Context;
use anyhow::{anyhow, Result};
use std::{env, process::Command};
#[cfg(target_os = "macos")]
use std::{fs, path::PathBuf};

const STARTUP_NAME: &str = "Wormhole";

pub fn is_enabled() -> Result<bool> {
    platform_is_enabled()
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    if enabled {
        platform_enable()
    } else {
        platform_disable()
    }
}

#[cfg(windows)]
fn platform_is_enabled() -> Result<bool> {
    let output = Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            STARTUP_NAME,
        ])
        .output()?;
    Ok(output.status.success())
}

#[cfg(windows)]
fn platform_enable() -> Result<()> {
    let exe = env::current_exe()?;
    let command = format!("\"{}\" --minimized", exe.display());
    let status = Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            STARTUP_NAME,
            "/t",
            "REG_SZ",
            "/d",
            &command,
            "/f",
        ])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("failed to enable Windows startup item"))
    }
}

#[cfg(windows)]
fn platform_disable() -> Result<()> {
    let status = Command::new("reg")
        .args([
            "delete",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            STARTUP_NAME,
            "/f",
        ])
        .status()?;
    if status.success() || !platform_is_enabled()? {
        Ok(())
    } else {
        Err(anyhow!("failed to disable Windows startup item"))
    }
}

#[cfg(target_os = "macos")]
fn platform_is_enabled() -> Result<bool> {
    Ok(startup_plist().is_file())
}

#[cfg(target_os = "macos")]
fn platform_enable() -> Result<()> {
    let exe = env::current_exe()?;
    let plist = startup_plist();
    if let Some(parent) = plist.parent() {
        fs::create_dir_all(parent)?;
    }
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>dev.wormhole.desktop.login</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>--minimized</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><false/>
</dict>
</plist>
"#,
        xml_escape(&exe.to_string_lossy())
    );
    fs::write(&plist, xml).with_context(|| format!("write {}", plist.display()))?;
    let _ = Command::new("launchctl").arg("unload").arg(&plist).output();
    let status = Command::new("launchctl").arg("load").arg(&plist).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("failed to enable macOS login item"))
    }
}

#[cfg(target_os = "macos")]
fn platform_disable() -> Result<()> {
    let plist = startup_plist();
    let _ = Command::new("launchctl").arg("unload").arg(&plist).output();
    if plist.exists() {
        fs::remove_file(&plist)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn startup_plist() -> PathBuf {
    let home = env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join("Library")
        .join("LaunchAgents")
        .join("dev.wormhole.desktop.login.plist")
}

#[cfg(target_os = "macos")]
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_is_enabled() -> Result<bool> {
    Ok(false)
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_enable() -> Result<()> {
    Err(anyhow!("startup is not supported on this platform"))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn platform_disable() -> Result<()> {
    Ok(())
}
