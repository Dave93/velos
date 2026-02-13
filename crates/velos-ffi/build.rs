use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let zig_dir = PathBuf::from(&manifest_dir).join("../../zig");
    let zig_dir = zig_dir.canonicalize().unwrap_or_else(|_| {
        PathBuf::from(&manifest_dir).join("../../zig")
    });
    let zig_dir_str = zig_dir.display().to_string();

    let lib_path = zig_dir.join("zig-out/lib/libvelos_core.a");
    if !lib_path.exists() {
        eprintln!("libvelos_core.a not found, running zig build...");
        let status = Command::new("zig")
            .arg("build")
            .current_dir(&zig_dir)
            .status()
            .expect("Failed to run zig build. Is zig installed?");
        if !status.success() {
            panic!("zig build failed with status: {}", status);
        }
    }

    println!("cargo:rustc-link-search=native={}/zig-out/lib", zig_dir_str);
    println!("cargo:rustc-link-lib=static=velos_core");
    println!("cargo:rerun-if-changed=../../zig/src/lib.zig");
}
