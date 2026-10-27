//! Windows only: embed the app icon and version info into `redrob-cad.exe`. Elsewhere a no-op.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/app-icon/redrob-cad.ico");
    println!("cargo:rerun-if-env-changed=REDROB_CAD_REQUIRE_WINRES");
    println!("cargo:rerun-if-env-changed=REDROB_CAD_BUILD_SHA");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    if std::path::Path::new("../../assets/app-icon/redrob-cad.ico").exists() {
        res.set_icon("../../assets/app-icon/redrob-cad.ico");
    }
    res.set("ProductName", "Redrob CAD")
        .set("FileDescription", "Redrob CAD computer-aided design")
        .set("CompanyName", "Redrob")
        .set("LegalCopyright", "Copyright (c) 2026 ArtCraft Team and the CADCraft contributors; Copyright (c) 2026 Janghoon Lee (Redrob) and contributors. MIT OR Apache-2.0.")
        .set("OriginalFilename", "redrob-cad.exe")
        .set("InternalName", "redrob-cad");
    if let Err(e) = res.compile() {
        if std::env::var_os("REDROB_CAD_REQUIRE_WINRES").is_some() {
            eprintln!("embedding Windows resources failed: {e}");
            std::process::exit(1);
        }
        println!("cargo:warning=redrob-cad.exe built without icon/version resources: {e}");
    }
}
