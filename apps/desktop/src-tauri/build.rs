use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    if let Err(err) = stage_native_host() {
        println!("cargo:warning=native host not staged: {err}");
    }
    tauri_build::build();
}

fn stage_native_host() -> Result<(), String> {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(|e| e.to_string())?);
    let root = manifest
        .join("../../..")
        .canonicalize()
        .map_err(|e| format!("repo root: {e}"))?;

    let candidates = [
        root.join("target/release/cookie-monster-native-host"),
        root.join("target/release/cookie-monster-native-host.exe"),
        root.join("target/debug/cookie-monster-native-host"),
        root.join("target/debug/cookie-monster-native-host.exe"),
    ];
    let src = candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| "build cookie-monster-native-host first".to_string())?;

    let dest_dir = manifest.join("binaries");
    fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let dest_name = if src.extension().and_then(|e| e.to_str()) == Some("exe") {
        "cookie-monster-native-host.exe"
    } else {
        "cookie-monster-native-host"
    };
    let dest = dest_dir.join(dest_name);
    copy_if_newer(&src, &dest)?;
    println!("cargo:rerun-if-changed={}", src.display());
    println!("cargo:rerun-if-changed={}", dest.display());
    Ok(())
}

fn copy_if_newer(src: &Path, dest: &Path) -> Result<(), String> {
    let needs = match (fs::metadata(src), fs::metadata(dest)) {
        (Ok(s), Ok(d)) => s.modified().ok() > d.modified().ok(),
        (Ok(_), Err(_)) => true,
        (Err(e), _) => return Err(e.to_string()),
    };
    if needs {
        fs::copy(src, dest).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(dest, fs::Permissions::from_mode(0o755));
        }
    }
    Ok(())
}
