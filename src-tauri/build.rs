use std::{
    env, fs,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    process::Command,
};

fn configured_tool_version(key: &str) -> String {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let path = manifest.join("..").join("config").join("bundled-tools.env");
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("Could not read {}: {error}", path.display()));
    let prefix = format!("{key}=");
    contents
        .lines()
        .find_map(|line| line.trim().strip_prefix(&prefix))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| panic!("{key} is missing from {}", path.display()))
        .to_owned()
}

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
    let mut candidates = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if name == "yt-dlp" {
        if let Some(local_data) = env::var_os("LOCALAPPDATA") {
            candidates.push(
                PathBuf::from(local_data)
                    .join("dev.dean.container")
                    .join("downloader")
                    .join("yt-dlp.exe"),
            );
        }
    }
    let path = candidates
        .into_iter()
        .find(|path| path.is_file())
        .unwrap_or_else(|| {
            panic!("{name}.exe was not found. Install it or set {override_name} to its full path.")
        });
    path.canonicalize().unwrap_or(path)
}

fn files_equal(left: &Path, right: &Path) -> bool {
    let Some((left_metadata, right_metadata)) =
        fs::metadata(left).ok().zip(fs::metadata(right).ok())
    else {
        return false;
    };
    if left_metadata.len() != right_metadata.len() {
        return false;
    }
    let Some((left_file, right_file)) = fs::File::open(left).ok().zip(fs::File::open(right).ok())
    else {
        return false;
    };
    let mut left_reader = BufReader::new(left_file);
    let mut right_reader = BufReader::new(right_file);
    let mut left_buffer = vec![0_u8; 1024 * 1024];
    let mut right_buffer = vec![0_u8; 1024 * 1024];
    loop {
        let Ok(left_count) = left_reader.read(&mut left_buffer) else {
            return false;
        };
        let Ok(right_count) = right_reader.read(&mut right_buffer) else {
            return false;
        };
        if left_count != right_count || left_buffer[..left_count] != right_buffer[..right_count] {
            return false;
        }
        if left_count == 0 {
            return true;
        }
    }
}

fn copy_if_changed(source: &Path, destination: &Path) {
    let unchanged = files_equal(source, destination);
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
        ("yt-dlp", "CONTAINER_YT_DLP"),
        ("deno", "CONTAINER_DENO"),
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
    let versions = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("config")
        .join("bundled-tools.env");
    println!("cargo:rerun-if-changed={}", versions.display());
    println!(
        "cargo:rustc-env=CONTAINER_FFMPEG_RUNTIME_VERSION={}",
        configured_tool_version("FFMPEG_VERSION")
    );
    println!("cargo:rerun-if-changed=tauri.conf.json");
    prepare_windows_sidecars();
    tauri_build::build()
}
