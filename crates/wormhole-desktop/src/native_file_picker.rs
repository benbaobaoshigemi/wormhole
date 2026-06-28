use std::path::PathBuf;

pub fn pick_files() -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new()
        .set_title("Send files with Wormhole")
        .pick_files()
}

pub fn pick_folder() -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new()
        .set_title("Send folder with Wormhole")
        .pick_folder()
        .map(|path| vec![path])
}
