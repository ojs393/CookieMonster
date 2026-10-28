
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::client::{listen_addr, token_path};

fn data_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "CookieMonster", "cookie-monster")
        .map(|d| d.data_dir().to_path_buf())
}

fn parse_listen() -> SocketAddr {
    listen_addr()
        .parse()
        .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 9735)))
}

fn host_reachable() -> bool {
    TcpStream::connect_timeout(&parse_listen(), Duration::from_millis(300)).is_ok()
}

fn push_host_name(out: &mut Vec<PathBuf>, path: PathBuf) {
    out.push(path.clone());
    // Windows cargo/tauri emit `.exe`; keep looking for both names everywhere.
    if path.extension().and_then(|e| e.to_str()) != Some("exe") {
        let mut exe = path.into_os_string();
        exe.push(".exe");
        out.push(PathBuf::from(exe));
    }
}

fn bundle_candidates(exe: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(macos_dir) = exe.parent() {
        push_host_name(&mut out, macos_dir.join("cookie-monster-native-host"));
        if let Some(contents) = macos_dir.parent() {
            push_host_name(
                &mut out,
                contents.join("Resources/binaries/cookie-monster-native-host"),
            );
            push_host_name(
                &mut out,
                contents.join("Resources/cookie-monster-native-host"),
            );
        }
    }
    out
}

fn candidate_bins() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        out.extend(bundle_candidates(&exe));
        if let Ok(canon) = exe.canonicalize() {
            out.extend(bundle_candidates(&canon));
        }
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        push_host_name(
            &mut out,
            PathBuf::from(manifest).join("binaries/cookie-monster-native-host"),
        );
    }
    if let Ok(cwd) = std::env::current_dir() {
        for rel in [
            "binaries/cookie-monster-native-host",
            "src-tauri/binaries/cookie-monster-native-host",
            "../../target/debug/cookie-monster-native-host",
            "../../target/release/cookie-monster-native-host",
            "../../../target/debug/cookie-monster-native-host",
            "../../../target/release/cookie-monster-native-host",
        ] {
            push_host_name(&mut out, cwd.join(rel));
        }
    }
    // Optional override for developers: COOKIE_MONSTER_ROOT=<repo>
    if let Ok(root) = std::env::var("COOKIE_MONSTER_ROOT") {
        let root = PathBuf::from(root);
        push_host_name(&mut out, root.join("target/release/cookie-monster-native-host"));
        push_host_name(&mut out, root.join("target/debug/cookie-monster-native-host"));
        push_host_name(
            &mut out,
            root.join("apps/desktop/src-tauri/binaries/cookie-monster-native-host"),
        );
    }
    out
}

fn resolve_host_bin() -> Result<PathBuf, String> {
    for path in candidate_bins() {
        if path.is_file() {
            return Ok(path.canonicalize().unwrap_or(path));
        }
    }
    Err(
        "Core host binary missing. Build cookie-monster-native-host (see README), then reinstall or restart."
            .into(),
    )
}

fn rotate_log_if_huge(path: &Path) {
    const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > MAX_LOG_BYTES {
            let bak = path.with_extension("log.1");
            let _ = std::fs::remove_file(&bak);
            let _ = std::fs::rename(path, &bak);
        }
    }
}

fn spawn_host(bin: &Path) -> Result<(), String> {
    let log_dir = data_dir().ok_or("no data directory")?;
    std::fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
    let log_path = log_dir.join("native-host.log");
    rotate_log_if_huge(&log_path);

    #[cfg(unix)]
    {
        let cmd = format!(
            "nohup \"{}\" --tcp-only >>\"{}\" 2>&1 & echo $!",
            bin.display(),
            log_path.display()
        );
        let output = Command::new("/bin/sh")
            .arg("-c")
            .arg(&cmd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .and_then(|c| c.wait_with_output())
            .map_err(|e| format!("failed to start host: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "host spawn failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        return Ok(());
    }

    #[cfg(not(unix))]
    {
        Command::new(bin)
            .arg("--tcp-only")
            .stdin(Stdio::null())
            .stdout(Stdio::from(
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path)
                    .map_err(|e| e.to_string())?,
            ))
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to start host ({}): {e}", bin.display()))?;
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EnsureHostResult {
    pub running: bool,
    pub started: bool,
    pub message: String,
}

pub fn ensure_running() -> Result<EnsureHostResult, String> {
    if host_reachable() {
        return Ok(EnsureHostResult {
            running: true,
            started: false,
            message: "Host already running".into(),
        });
    }

    let bin = resolve_host_bin()?;
    spawn_host(&bin)?;

    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(100));
        if host_reachable() {
            let _ = token_path();
            return Ok(EnsureHostResult {
                running: true,
                started: true,
                message: "Core host started".into(),
            });
        }
    }

    Err(format!(
        "Host did not become ready on {}. Check Settings → Restart core.",
        listen_addr()
    ))
}
