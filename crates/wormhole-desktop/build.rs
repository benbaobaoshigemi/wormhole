fn main() {
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../../assets/wormhole/wormhole.ico");
        resource.set("FileDescription", "Wormhole");
        resource.set("ProductName", "Wormhole");
        resource
            .compile()
            .expect("failed to embed Wormhole Windows resources");
    }
}
