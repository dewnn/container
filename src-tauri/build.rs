use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn locate(name: &str, override_name: &str) -> PathBuf {
    if let Some(path) = env::var_os(override_name).map(PathBuf::from) {
        if path.is_file() {
            return path.canonicalize().unwrap_or(path);
        }
        panic!(
            "{override_name} does not point to a file: {}",
            path.display()
        );
    }
    let output = Command::new("where.exe")
        .arg(format!("{name}.exe"))
        .output()
        .unwrap_or_else(|error| panic!("Could not search for {name}: {error}"));
    let path = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .unwrap_or_else(|| {
            panic!(
                "{name}.exe was not found. Install the full FFmpeg build or set {override_name}."
            )
        });
    path.canonicalize().unwrap_or(path)
}

fn copy_if_changed(source: &Path, destination: &Path) {
    let unchanged = fs::metadata(source)
        .ok()
        .zip(fs::metadata(destination).ok())
        .is_some_and(|(left, right)| left.len() == right.len());
    if !unchanged {
        fs::create_dir_all(destination.parent().expect("sidecar directory"))
            .expect("create sidecar directory");
        fs::copy(source, destination).unwrap_or_else(|error| {
            panic!(
                "Could not copy {} to {}: {error}",
                source.display(),
                destination.display()
            )
        });
    }
}

fn prepare_windows_sidecars() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let target = env::var("TARGET").expect("TARGET is set by Cargo");
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    for (name, override_name) in [
        ("ffmpeg", "CONTAINER_FFMPEG"),
        ("ffprobe", "CONTAINER_FFPROBE"),
    ] {
        let source = locate(name, override_name);
        let destination = manifest
            .join("binaries")
            .join(format!("{name}-{target}.exe"));
        copy_if_changed(&source, &destination);
        println!("cargo:rerun-if-env-changed={override_name}");
        println!("cargo:rerun-if-changed={}", source.display());
    }
}

fn main() {
    println!("cargo:rerun-if-changed=tauri.conf.json");
    prepare_windows_sidecars();
    tauri_build::build()
}
