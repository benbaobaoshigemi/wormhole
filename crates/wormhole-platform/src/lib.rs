#[cfg(windows)]
mod windows_clipboard;

#[cfg(target_os = "macos")]
mod macos_clipboard;

#[cfg(windows)]
pub use windows_clipboard::SystemClipboard;

#[cfg(target_os = "macos")]
pub use macos_clipboard::SystemClipboard;

#[cfg(not(any(windows, target_os = "macos")))]
compile_error!("wormhole-platform clipboard is implemented only for Windows and macOS");
