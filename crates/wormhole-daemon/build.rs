fn main() {
    #[cfg(target_os = "macos")]
    {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        let plist = std::path::Path::new(&manifest_dir).join("macos/Info.plist");
        println!("cargo:rerun-if-changed=macos/Info.plist");
        println!(
            "cargo:rustc-link-arg-bin=wormhole-daemon=-Wl,-sectcreate,__TEXT,__info_plist,{}",
            plist.display()
        );
    }
}
