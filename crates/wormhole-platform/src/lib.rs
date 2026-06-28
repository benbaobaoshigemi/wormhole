#[cfg(windows)]
mod windows_clipboard;

#[cfg(target_os = "macos")]
mod macos_clipboard;

#[cfg(windows)]
pub use windows_clipboard::SystemClipboard;

#[cfg(target_os = "macos")]
pub use macos_clipboard::SystemClipboard;

#[cfg(not(any(windows, target_os = "macos")))]
pub struct SystemClipboard {
    text: Option<String>,
    png: Option<Vec<u8>>,
}

#[cfg(not(any(windows, target_os = "macos")))]
impl SystemClipboard {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            text: None,
            png: None,
        })
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
impl wormhole_core::ClipboardPort for SystemClipboard {
    fn read_text(&mut self) -> anyhow::Result<Option<String>> {
        Ok(self.text.clone())
    }

    fn write_text(&mut self, text: &str) -> anyhow::Result<()> {
        self.text = Some(text.to_string());
        Ok(())
    }

    fn read_png(&mut self) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(self.png.clone())
    }

    fn write_png(&mut self, png: &[u8]) -> anyhow::Result<()> {
        self.png = Some(png.to_vec());
        Ok(())
    }
}
