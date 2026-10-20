//! Embeds the application icon, version information and manifest into LibreLinkUp.exe.

fn main() {
    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=assets/LibreLinkUp.exe.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let version = env!("CARGO_PKG_VERSION");
    let mut res = winresource::WindowsResource::new();
    // Cross-compiling from Linux with MinGW: use the target-prefixed resource tools.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") && !cfg!(windows) {
        res.set_windres_path("x86_64-w64-mingw32-windres");
        res.set_ar_path("x86_64-w64-mingw32-ar");
    }
    res.set_icon("assets/app.ico")
        .set_manifest_file("assets/LibreLinkUp.exe.manifest")
        .set("ProductName", "LibreLinkUp Rust Desktop")
        .set("FileDescription", "LibreLinkUp Rust Desktop — blood glucose monitor")
        .set("CompanyName", "Russell Milliner")
        .set("LegalCopyright", "Copyright © Russell Milliner. Apache License 2.0.")
        .set("OriginalFilename", "LibreLinkUp.exe")
        .set("ProductVersion", version)
        .set("FileVersion", version);
    if let Err(e) = res.compile() {
        panic!("failed to compile Windows resources: {e}");
    }
}
