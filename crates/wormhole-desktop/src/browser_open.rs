use anyhow::Result;

pub fn open_control_center(url: &str) -> Result<()> {
    open::that(url)?;
    Ok(())
}

pub fn open_path(path: impl AsRef<std::path::Path>) -> Result<()> {
    open::that(path.as_ref())?;
    Ok(())
}
