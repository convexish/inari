#![forbid(unsafe_code)]
//! Windows builds link the app icon, compiled by zig's resource compiler.

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let ico = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/inari.ico");
    println!("cargo:rerun-if-changed={ico}");
    let out = std::env::var("OUT_DIR").unwrap();
    let (rc, res) = (
        Path::new(&out).join("inari.rc"),
        Path::new(&out).join("inari.res"),
    );
    std::fs::write(&rc, format!("1 ICON \"{ico}\"\n")).unwrap();
    let status = Command::new("zig")
        .arg("rc")
        .arg("/fo")
        .args([&res, &rc])
        .status()
        .expect("zig is required to build for Windows");
    assert!(status.success(), "zig rc failed");
    println!("cargo:rustc-link-arg-bins={}", res.display());
}
