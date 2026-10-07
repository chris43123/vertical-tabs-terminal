//! Embeds the app icon and version info in the Windows executable.

fn main() {
    println!("cargo:rerun-if-changed=packaging/icons/vtt.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("packaging/icons/vtt.ico")
            .set("ProductName", "vtt")
            .set("FileDescription", "vtt - Vertical Tabs Terminal");
        if let Err(err) = res.compile() {
            println!("cargo:warning=couldn't embed the Windows icon: {err}");
        }
    }
}
