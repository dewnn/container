use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=tauri.conf.json");
    ensure_windows_sidecars();
    tauri_build::build()
}

// A release must remain usable on a PC that has never installed FFmpeg.
// Tauri expects external binaries to carry the Rust target suffix while
// building, then installs them beside the application under their plain name.
fn ensure_windows_sidecars() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_else(|_| "x86_64".into());
    let target = format!("{arch}-pc-windows-msvc");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let destination = manifest.join("binaries");
    fs::create_dir_all(&destination).expect("create sidecar directory");

    for tool in ["ffmpeg", "ffprobe"] {
        let output = destination.join(format!("{tool}-{target}.exe"));
        if output.metadata().map(|m| m.len() > 0).unwrap_or(false) {
            continue;
        }

        let found = Command::new("where.exe")
            .arg(format!("{tool}.exe"))
            .output()
            .unwrap_or_else(|error| panic!("could not search for {tool}: {error}"));
        assert!(
            found.status.success(),
            "{tool}.exe is required to create a self-contained build"
        );
        let source = String::from_utf8_lossy(&found.stdout)
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(PathBuf::from)
            .expect("where.exe returned no path");
        fs::copy(&source, &output)
            .unwrap_or_else(|error| panic!("could not bundle {}: {error}", source.display()));
    }
}
