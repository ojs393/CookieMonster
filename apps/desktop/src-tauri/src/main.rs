#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod client;
mod host;
mod setup;

use std::time::Duration;

use client::{fetch_snapshot, listen_addr, mode_label, token_path, Snapshot};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

fn tray_icon() -> Result<Image<'static>, Box<dyn std::error::Error>> {
    const TRAY_2X: &[u8] = include_bytes!("../icons/wendy.h@example.net");
    const TRAY_1X: &[u8] = include_bytes!("../icons/tray-template.png");
    Image::from_bytes(TRAY_2X)
        .or_else(|_| Image::from_bytes(TRAY_1X))
        .map_err(|e| e.into())
}

fn snapshot() -> Snapshot {
    match token_path() {
        Some(path) => fetch_snapshot(&listen_addr(), &path),
        None => Snapshot::disconnected("no data directory"),
    }
}

#[tauri::command]
fn get_snapshot() -> Snapshot {
    let snap = snapshot();
    if snap.connected {
        return snap;
    }
    let _ = host::ensure_running();
    snapshot()
}

#[tauri::command]
fn ensure_host() -> Result<host::EnsureHostResult, String> {
    let result = host::ensure_running()?;
    let mut state = setup::load_state();
    state.host_done = true;
    let _ = setup::save_state(&state);
    Ok(result)
}

#[tauri::command]
fn set_mode(mode: String) -> Result<String, String> {
    let _ = host::ensure_running();
    let path = token_path().ok_or_else(|| "no data directory".to_string())?;
    client::set_mode(&listen_addr(), &path, &mode)
}

#[tauri::command]
fn set_allowlist(hosts_csv: String) -> Result<(), String> {
    let _ = host::ensure_running();
    let path = token_path().ok_or_else(|| "no data directory".to_string())?;
    let hosts: Vec<String> = hosts_csv
        .split(|c| c == ',' || c == '\n' || c == ' ')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    client::set_allowlist_hosts(&listen_addr(), &path, &hosts)
}

#[tauri::command]
fn set_system_shield(enabled: bool) -> Result<bool, String> {
    let _ = host::ensure_running();
    let path = token_path().ok_or_else(|| "no data directory".to_string())?;
    client::set_system_shield(&listen_addr(), &path, enabled)
}

#[tauri::command]
fn get_allowlist() -> Result<Vec<String>, String> {
    let _ = host::ensure_running();
    let path = token_path().ok_or_else(|| "no data directory".to_string())?;
    client::get_allowlist(&listen_addr(), &path)
}

#[tauri::command]
fn get_setup_state() -> setup::SetupState {
    setup::load_state()
}

#[tauri::command]
fn save_setup_state(state: setup::SetupState) -> Result<(), String> {
    setup::save_state(&state)
}

#[tauri::command]
fn install_local_ca() -> Result<String, String> {
    let _ = host::ensure_running();
    for _ in 0..20 {
        if setup::ca_cert_path().is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    setup::install_ca()
}

#[tauri::command]
fn set_system_proxy(enabled: bool) -> Result<String, String> {
    setup::set_system_proxy(enabled)
}

/// Backward-compatible alias used by older UI builds.
#[tauri::command]
fn set_macos_proxy(enabled: bool) -> Result<String, String> {
    setup::set_system_proxy(enabled)
}

#[tauri::command]
fn open_extension_folder() -> Result<String, String> {
    for dir in setup::extension_dirs() {
        let path = std::path::Path::new(&dir);
        if path.is_dir() {
            setup::open_path(&dir)?;
            return Ok(dir);
        }
    }
    Err("Extension folder not found. Expected extensions/chromium next to the repo.".into())
}

#[tauri::command]
fn reveal_ca_cert() -> Result<String, String> {
    let path = setup::ca_cert_path()?;
    let s = path.display().to_string();
    setup::open_path(path.parent().unwrap_or(path.as_path()).to_str().unwrap_or(&s))?;
    Ok(s)
}

#[tauri::command]
fn clear_events() -> Result<u64, String> {
    let _ = host::ensure_running();
    let path = token_path().ok_or_else(|| "no data directory".to_string())?;
    client::clear_events(&listen_addr(), &path)
}

#[tauri::command]
fn get_store_stats() -> Result<client::StoreStats, String> {
    let _ = host::ensure_running();
    let path = token_path().ok_or_else(|| "no data directory".to_string())?;
    client::get_store_stats(&listen_addr(), &path)
}

#[tauri::command]
fn set_retention_days(days: u32) -> Result<client::StoreStats, String> {
    let _ = host::ensure_running();
    let path = token_path().ok_or_else(|| "no data directory".to_string())?;
    client::set_retention_days(&listen_addr(), &path, days)
}

#[tauri::command]
fn reset_setup() -> Result<setup::SetupState, String> {
    setup::reset_setup()
}

#[tauri::command]
fn mark_setup_complete() -> Result<setup::SetupState, String> {
    setup::mark_completed(setup::load_state())
}

#[tauri::command]
fn open_data_dir() -> Result<String, String> {
    let dir = setup::data_dir_path()?;
    let s = dir.display().to_string();
    setup::open_path(&s)?;
    Ok(s)
}

fn toggle_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn tray_title(snapshot: &Snapshot) -> String {
    match &snapshot.mode {
        Some(mode) if snapshot.connected => format!("🍪 {}", mode_label(mode)),
        _ => "🍪 offline".to_string(),
    }
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            get_snapshot,
            ensure_host,
            set_mode,
            set_allowlist,
            set_system_shield,
            get_allowlist,
            get_setup_state,
            save_setup_state,
            install_local_ca,
            set_system_proxy,
            set_macos_proxy,
            open_extension_folder,
            reveal_ca_cert,
            clear_events,
            get_store_stats,
            set_retention_days,
            reset_setup,
            mark_setup_complete,
            open_data_dir,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Regular);

            match host::ensure_running() {
                Ok(r) => eprintln!("cookie-monster-desktop: {}", r.message),
                Err(e) => eprintln!("cookie-monster-desktop: host start: {e}"),
            }

            let status_item = MenuItem::with_id(app, "status", "Mode: …", false, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "Show panel", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Cookie Monster", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&status_item, &show, &quit])?;

            let tray_image = tray_icon().map_err(|e| e.to_string())?;
            let tray = TrayIconBuilder::with_id("main")
                .icon(tray_image)
                .icon_as_template(true)
                .tooltip("Cookie Monster")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => toggle_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_window(tray.app_handle());
                    }
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }

            std::thread::spawn(move || {
                let mut ticks: u64 = 0;
                loop {
                    let mut snap = snapshot();
                    if !snap.connected {
                        let _ = host::ensure_running();
                        snap = snapshot();
                    }
                    let _ = tray.set_tooltip(Some(tray_title(&snap)));
                    let _ = status_item.set_text(match (&snap.mode, snap.connected) {
                        (Some(mode), true) => format!("Mode: {}", mode_label(mode)),
                        _ => "Host offline — restarting…".to_string(),
                    });
                    ticks += 1;
                    if ticks % 300 == 0 {
                        let _ = host::ensure_running();
                    }
                    std::thread::sleep(Duration::from_secs(1));
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Cookie Monster desktop");
}
