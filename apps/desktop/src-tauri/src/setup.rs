
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const SETUP_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SetupState {
    pub schema_version: u32,
    pub welcome_done: bool,
    pub host_done: bool,
    pub ca_done: bool,
    pub extension_done: bool,
    pub system_shield_done: bool,
    pub completed: bool,
    pub completed_at_ms: Option<u64>,
    pub last_app_version: Option<String>,
}

impl Default for SetupState {
    fn default() -> Self {
        Self {
            schema_version: SETUP_SCHEMA_VERSION,
            welcome_done: false,
            host_done: false,
            ca_done: false,
            extension_done: false,
            system_shield_done: false,
            completed: false,
            completed_at_ms: None,
            last_app_version: None,
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn state_path() -> Result<PathBuf, String> {
    let dir = directories::ProjectDirs::from("com", "CookieMonster", "cookie-monster")
        .ok_or_else(|| "no data directory".to_string())?
        .data_dir()
        .to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("setup.json"))
}

pub fn data_dir_path() -> Result<PathBuf, String> {
    directories::ProjectDirs::from("com", "CookieMonster", "cookie-monster")
        .map(|d| d.data_dir().to_path_buf())
        .ok_or_else(|| "no data directory".to_string())
}

pub fn load_state() -> SetupState {
    let Ok(path) = state_path() else {
        return SetupState::default();
    };
    let mut state: SetupState = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    if state.schema_version < SETUP_SCHEMA_VERSION {
        state.schema_version = SETUP_SCHEMA_VERSION;
        let _ = save_state(&state);
    }

    let ver = app_version();
    if state.last_app_version.as_deref() != Some(ver.as_str()) {
        state.last_app_version = Some(ver);
        let _ = save_state(&state);
    }

    state
}

pub fn save_state(state: &SetupState) -> Result<(), String> {
    let path = state_path()?;
    let mut to_save = state.clone();
    to_save.schema_version = SETUP_SCHEMA_VERSION;
    if to_save.completed && to_save.completed_at_ms.is_none() {
        to_save.completed_at_ms = Some(now_ms());
    }
    to_save.last_app_version = Some(app_version());
    let body = serde_json::to_string_pretty(&to_save).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &body).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

pub fn mark_completed(mut state: SetupState) -> Result<SetupState, String> {
    state.completed = true;
    state.welcome_done = true;
    state.host_done = true;
    state.completed_at_ms = Some(now_ms());
    save_state(&state)?;
    Ok(state)
}

pub fn reset_setup() -> Result<SetupState, String> {
    let state = SetupState::default();
    save_state(&state)?;
    Ok(state)
}

pub fn ca_cert_path() -> Result<PathBuf, String> {
    let dir = data_dir_path()?.join("local-ca").join("cookie-monster-ca.cer");
    if !dir.exists() {
        return Err(
            "Local CA not created yet — start the host first (it creates the certificate).".into(),
        );
    }
    Ok(dir)
}

fn mark_ca_done() {
    let mut state = load_state();
    state.ca_done = true;
    let _ = save_state(&state);
}

fn mark_proxy_done(enable: bool) {
    if !enable {
        return;
    }
    let mut state = load_state();
    state.system_shield_done = true;
    let _ = save_state(&state);
}

pub fn install_ca() -> Result<String, String> {
    let cert = ca_cert_path()?;

    #[cfg(target_os = "macos")]
    {
        let cert_str = cert.display().to_string();
        let script = format!(
            r#"do shell script "security add-trusted-cert -r trustRoot -k \"$HOME/Library/Keychains/login.keychain-db\" '{}'" with administrator privileges"#,
            cert_str.replace('\'', "'\\''")
        );
        let output = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| format!("osascript: {e}"))?;
        if output.status.success() {
            mark_ca_done();
            return Ok("Local CA trusted in your login keychain.".into());
        }
        let err = String::from_utf8_lossy(&output.stderr);
        let fallback = Command::new("security")
            .args(["add-trusted-cert", "-r", "trustRoot", "-k"])
            .arg(format!(
                "{}/Library/Keychains/login.keychain-db",
                std::env::var("HOME").unwrap_or_default()
            ))
            .arg(&cert)
            .output()
            .map_err(|e| format!("security: {e}"))?;
        if fallback.status.success() {
            mark_ca_done();
            return Ok("Local CA trusted in your login keychain.".into());
        }
        return Err(format!(
            "Could not trust CA. {}\n{}",
            err.trim(),
            String::from_utf8_lossy(&fallback.stderr).trim()
        ));
    }

    #[cfg(target_os = "windows")]
    {
        let output = Command::new("certutil")
            .args(["-user", "-addstore", "Root"])
            .arg(&cert)
            .output()
            .map_err(|e| format!("certutil: {e}"))?;
        if output.status.success() {
            mark_ca_done();
            return Ok("Local CA trusted in the current-user Root store.".into());
        }
        let _ = open_path(
            cert.parent()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| cert.display().to_string())
                .as_str(),
        );
        return Err(format!(
            "certutil failed ({}). Import cookie-monster-ca.cer into Trusted Root manually.",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // Prefer NSS user DB (Chrome/Chromium/Firefox-friendly) when certutil exists.
        if let Ok(home) = std::env::var("HOME") {
            let nss = PathBuf::from(&home).join(".pki/nssdb");
            if Command::new("certutil").arg("-h").output().is_ok() {
                let _ = std::fs::create_dir_all(&nss);
                let add = Command::new("certutil")
                    .args(["-d", &format!("sql:{}", nss.display()), "-A", "-t", "C,,", "-n", "Cookie Monster Local CA", "-i"])
                    .arg(&cert)
                    .output();
                if let Ok(out) = add {
                    if out.status.success() {
                        mark_ca_done();
                        return Ok("Local CA trusted in ~/.pki/nssdb (NSS).".into());
                    }
                }
            }
        }

        // System store when the user can write ca-certificates.
        let dest_dir = PathBuf::from("/usr/local/share/ca-certificates");
        if dest_dir.is_dir() || std::fs::create_dir_all(&dest_dir).is_ok() {
            let dest = dest_dir.join("cookie-monster-ca.crt");
            if std::fs::copy(&cert, &dest).is_ok() {
                let upd = Command::new("update-ca-certificates").output();
                if upd.map(|o| o.status.success()).unwrap_or(false) {
                    mark_ca_done();
                    return Ok("Local CA installed via update-ca-certificates.".into());
                }
            }
        }

        let folder = cert
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| cert.display().to_string());
        let _ = open_path(&folder);
        mark_ca_done();
        Ok(format!(
            "Opened {folder}. Import cookie-monster-ca.cer into your browser/OS trust store, then continue."
        ))
    }
}

pub fn set_system_proxy(enable: bool) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        let service =
            std::env::var("COOKIE_MONSTER_NETWORK_SERVICE").unwrap_or_else(|_| "Wi-Fi".into());
        let script = if enable {
            format!(
                r#"do shell script "networksetup -setwebproxy '{service}' 127.0.0.1 8877 && networksetup -setsecurewebproxy '{service}' 127.0.0.1 8877 && networksetup -setwebproxystate '{service}' on && networksetup -setsecurewebproxystate '{service}' on" with administrator privileges"#
            )
        } else {
            format!(
                r#"do shell script "networksetup -setwebproxystate '{service}' off && networksetup -setsecurewebproxystate '{service}' off" with administrator privileges"#
            )
        };
        let output = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| format!("osascript: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "Proxy change failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        mark_proxy_done(enable);
        return Ok(if enable {
            format!("HTTP/HTTPS proxy on {service} → 127.0.0.1:8877")
        } else {
            format!("HTTP/HTTPS proxy disabled on {service}")
        });
    }

    #[cfg(target_os = "windows")]
    {
        let enable_val = if enable { "1" } else { "0" };
        let proxy_server = "127.0.0.1:8877";
        let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";
        let r1 = Command::new("reg")
            .args(["add", key, "/v", "ProxyEnable", "/t", "REG_DWORD", "/d", enable_val, "/f"])
            .output()
            .map_err(|e| format!("reg: {e}"))?;
        if !r1.status.success() {
            return Err(format!(
                "ProxyEnable failed: {}",
                String::from_utf8_lossy(&r1.stderr).trim()
            ));
        }
        if enable {
            let r2 = Command::new("reg")
                .args(["add", key, "/v", "ProxyServer", "/t", "REG_SZ", "/d", proxy_server, "/f"])
                .output()
                .map_err(|e| format!("reg: {e}"))?;
            if !r2.status.success() {
                return Err(format!(
                    "ProxyServer failed: {}",
                    String::from_utf8_lossy(&r2.stderr).trim()
                ));
            }
        }
        // Notify WinINET of settings change (best-effort).
        let _ = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "[void][System.Reflection.Assembly]::LoadWithPartialName('System'); Add-Type -Namespace WinI -Name N -MemberDefinition '[DllImport(\"wininet.dll\")] public static extern bool InternetSetOption(IntPtr h, int o, IntPtr b, int l);'; [WinI.N]::InternetSetOption([IntPtr]::Zero, 39, [IntPtr]::Zero, 0); [WinI.N]::InternetSetOption([IntPtr]::Zero, 37, [IntPtr]::Zero, 0)",
            ])
            .output();
        mark_proxy_done(enable);
        return Ok(if enable {
            "HTTP/HTTPS user proxy → 127.0.0.1:8877 (WinINET)".into()
        } else {
            "HTTP/HTTPS user proxy disabled (WinINET)".into()
        });
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if Command::new("gsettings").arg("--version").output().is_ok() {
            if enable {
                let steps = [
                    ("org.gnome.system.proxy", "mode", "manual"),
                    ("org.gnome.system.proxy.http", "host", "127.0.0.1"),
                    ("org.gnome.system.proxy.https", "host", "127.0.0.1"),
                ];
                for (schema, key, value) in steps {
                    let out = Command::new("gsettings")
                        .args(["set", schema, key, value])
                        .output()
                        .map_err(|e| format!("gsettings: {e}"))?;
                    if !out.status.success() {
                        return Err(format!(
                            "gsettings {schema} {key} failed: {}",
                            String::from_utf8_lossy(&out.stderr).trim()
                        ));
                    }
                }
                for (schema, port) in [
                    ("org.gnome.system.proxy.http", "8877"),
                    ("org.gnome.system.proxy.https", "8877"),
                ] {
                    let out = Command::new("gsettings")
                        .args(["set", schema, "port", port])
                        .output()
                        .map_err(|e| format!("gsettings: {e}"))?;
                    if !out.status.success() {
                        return Err(format!(
                            "gsettings {schema} port failed: {}",
                            String::from_utf8_lossy(&out.stderr).trim()
                        ));
                    }
                }
                mark_proxy_done(true);
                return Ok("GNOME HTTP/HTTPS proxy → 127.0.0.1:8877".into());
            }
            let out = Command::new("gsettings")
                .args(["set", "org.gnome.system.proxy", "mode", "none"])
                .output()
                .map_err(|e| format!("gsettings: {e}"))?;
            if !out.status.success() {
                return Err(format!(
                    "gsettings disable failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ));
            }
            return Ok("GNOME system proxy disabled".into());
        }

        if enable {
            if let Ok(dir) = data_dir_path() {
                let _ = open_path(&dir.display().to_string());
            }
            return Err(
                "No gsettings (GNOME) helper. Set your desktop/browser HTTP(S) proxy to 127.0.0.1:8877 manually."
                    .into(),
            );
        }
        Ok("Proxy left unchanged (no gsettings). Disable 127.0.0.1:8877 in your desktop settings if set.".into())
    }
}

pub fn open_path(path: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub fn extension_dirs() -> Vec<String> {
    let mut dirs = Vec::new();
    if let Ok(root) = std::env::var("COOKIE_MONSTER_ROOT") {
        let root = PathBuf::from(root);
        dirs.push(root.join("extensions/chromium").display().to_string());
        dirs.push(root.join("extensions/firefox").display().to_string());
    }
    if let Ok(exe) = std::env::current_exe() {
        let mut cur = exe.parent().map(PathBuf::from);
        for _ in 0..8 {
            let Some(dir) = cur else { break };
            let chromium = dir.join("extensions/chromium");
            if chromium.is_dir() {
                dirs.push(chromium.display().to_string());
                dirs.push(dir.join("extensions/firefox").display().to_string());
                break;
            }
            cur = dir.parent().map(PathBuf::from);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        for rel in [
            "extensions/chromium",
            "../../extensions/chromium",
            "../../../extensions/chromium",
        ] {
            let p = cwd.join(rel);
            if p.is_dir() {
                dirs.push(p.display().to_string());
            }
        }
    }
    dirs
}
