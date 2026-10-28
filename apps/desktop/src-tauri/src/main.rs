#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod client;
mod host;
mod setup;

use std::sync::Mutex;
use std::time::Duration;

use client::{fetch_snapshot, listen_addr, mode_label, token_path, Snapshot};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

struct TrayMenuHandles {
    status: MenuItem<Wry>,
    host_line: MenuItem<Wry>,
    shield_line: MenuItem<Wry>,
    mode_observe: CheckMenuItem<Wry>,
    mode_moderate: CheckMenuItem<Wry>,
    mode_monster: CheckMenuItem<Wry>,
    shield: CheckMenuItem<Wry>,
}

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

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn hide_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

fn navigate(app: &AppHandle, tab: &str) {
    show_window(app);
    let _ = app.emit("navigate-tab", serde_json::json!({ "tab": tab }));
}

fn navigate_explore(app: &AppHandle, group: &str) {
    show_window(app);
    let _ = app.emit(
        "navigate-tab",
        serde_json::json!({ "tab": "explore", "group": group }),
    );
}

fn tray_title(snapshot: &Snapshot) -> String {
    match (&snapshot.mode, snapshot.connected, snapshot.system_shield) {
        (Some(mode), true, true) => format!("ON · {} · shield", mode_label(mode)),
        (Some(mode), true, false) => format!("ON · {}", mode_label(mode)),
        _ => "OFF · host offline".to_string(),
    }
}

fn refresh_tray_menu(handles: &TrayMenuHandles, snap: &Snapshot) {
    let online = snap.connected;
    let mode = snap.mode.as_deref().unwrap_or("");
    let status_text = if online {
        format!("● ON — {}", mode_label(mode))
    } else {
        "○ OFF — host offline".to_string()
    };
    let _ = handles.status.set_text(status_text.as_str());
    let _ = handles.host_line.set_text(if online {
        "Core host: running"
    } else {
        "Core host: offline"
    });
    let shield_line = if snap.system_shield {
        "System shield: ON"
    } else {
        "System shield: OFF"
    };
    let _ = handles.shield_line.set_text(shield_line);
    let _ = handles.mode_observe.set_checked(online && mode == "observe");
    let _ = handles.mode_moderate.set_checked(online && mode == "moderate");
    let _ = handles
        .mode_monster
        .set_checked(online && mode == "cookie_monster");
    let _ = handles.shield.set_checked(online && snap.system_shield);
    let shield_label = if snap.system_shield {
        "System shield ON"
    } else {
        "System shield OFF"
    };
    let _ = handles.shield.set_text(shield_label);
    let _ = handles.mode_observe.set_enabled(online);
    let _ = handles.mode_moderate.set_enabled(online);
    let _ = handles.mode_monster.set_enabled(online);
    let _ = handles.shield.set_enabled(online);
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
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            match host::ensure_running() {
                Ok(r) => eprintln!("cookie-monster-desktop: {}", r.message),
                Err(e) => eprintln!("cookie-monster-desktop: host start: {e}"),
            }

            let status = MenuItem::with_id(app, "status", "○ OFF — …", false, None::<&str>)?;
            let host_line =
                MenuItem::with_id(app, "host_line", "Core host: …", false, None::<&str>)?;
            let shield_line =
                MenuItem::with_id(app, "shield_line", "System shield: …", false, None::<&str>)?;

            let open_panel =
                MenuItem::with_id(app, "open_panel", "Open Cookie Monster", true, None::<&str>)?;
            let open_control =
                MenuItem::with_id(app, "open_control", "Open Control", true, None::<&str>)?;
            let open_activity =
                MenuItem::with_id(app, "open_activity", "Open Activity", true, None::<&str>)?;
            let open_explore =
                MenuItem::with_id(app, "open_explore", "Open Explore", true, None::<&str>)?;
            let open_explore_host = MenuItem::with_id(
                app,
                "open_explore_host",
                "Explore by host",
                true,
                None::<&str>,
            )?;
            let open_explore_cat = MenuItem::with_id(
                app,
                "open_explore_cat",
                "Explore by category",
                true,
                None::<&str>,
            )?;
            let open_setup =
                MenuItem::with_id(app, "open_setup", "Open Setup", true, None::<&str>)?;
            let open_settings =
                MenuItem::with_id(app, "open_settings", "Open Settings", true, None::<&str>)?;
            let hide_panel =
                MenuItem::with_id(app, "hide_panel", "Hide panel", true, None::<&str>)?;

            let mode_observe =
                CheckMenuItem::with_id(app, "mode_observe", "Observe", true, false, None::<&str>)?;
            let mode_moderate = CheckMenuItem::with_id(
                app,
                "mode_moderate",
                "Moderate",
                true,
                false,
                None::<&str>,
            )?;
            let mode_monster = CheckMenuItem::with_id(
                app,
                "mode_monster",
                "Cookie Monster",
                true,
                false,
                None::<&str>,
            )?;
            let mode_menu = Submenu::with_id_and_items(
                app,
                "mode_menu",
                "Mode",
                true,
                &[&mode_observe, &mode_moderate, &mode_monster],
            )?;

            let shield = CheckMenuItem::with_id(
                app,
                "toggle_shield",
                "System shield",
                true,
                false,
                None::<&str>,
            )?;
            let proxy_on =
                MenuItem::with_id(app, "proxy_on", "Turn proxy ON", true, None::<&str>)?;
            let proxy_off =
                MenuItem::with_id(app, "proxy_off", "Turn proxy OFF", true, None::<&str>)?;
            let restart =
                MenuItem::with_id(app, "restart_host", "Restart core", true, None::<&str>)?;
            let clear_log =
                MenuItem::with_id(app, "clear_log", "Clear activity log", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Cookie Monster", true, None::<&str>)?;
            let sep1 = PredefinedMenuItem::separator(app)?;
            let sep2 = PredefinedMenuItem::separator(app)?;
            let sep3 = PredefinedMenuItem::separator(app)?;
            let sep4 = PredefinedMenuItem::separator(app)?;

            let menu = Menu::with_items(
                app,
                &[
                    &status,
                    &host_line,
                    &shield_line,
                    &sep1,
                    &open_panel,
                    &open_control,
                    &open_activity,
                    &open_explore,
                    &open_explore_host,
                    &open_explore_cat,
                    &open_setup,
                    &open_settings,
                    &hide_panel,
                    &sep2,
                    &mode_menu,
                    &shield,
                    &proxy_on,
                    &proxy_off,
                    &sep3,
                    &restart,
                    &clear_log,
                    &sep4,
                    &quit,
                ],
            )?;

            let handles = TrayMenuHandles {
                status: status.clone(),
                host_line: host_line.clone(),
                shield_line: shield_line.clone(),
                mode_observe: mode_observe.clone(),
                mode_moderate: mode_moderate.clone(),
                mode_monster: mode_monster.clone(),
                shield: shield.clone(),
            };
            app.manage(Mutex::new(handles));

            let tray_image = tray_icon().map_err(|e| e.to_string())?;
            let tray = TrayIconBuilder::with_id("main")
                .icon(tray_image)
                .icon_as_template(true)
                .tooltip("Cookie Monster")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| {
                    let id = event.id().as_ref();
                    match id {
                        "open_panel" => {
                            show_window(app);
                            let _ = app.emit(
                                "navigate-tab",
                                serde_json::json!({ "tab": "control" }),
                            );
                        }
                        "open_control" => navigate(app, "control"),
                        "open_activity" => navigate(app, "activity"),
                        "open_explore" => navigate_explore(app, "host"),
                        "open_explore_host" => navigate_explore(app, "host"),
                        "open_explore_cat" => navigate_explore(app, "bucket"),
                        "open_setup" => navigate(app, "setup"),
                        "open_settings" => navigate(app, "settings"),
                        "hide_panel" => hide_window(app),
                        "mode_observe" => {
                            let _ = set_mode("observe".into());
                        }
                        "mode_moderate" => {
                            let _ = set_mode("moderate".into());
                        }
                        "mode_monster" => {
                            let _ = set_mode("cookie_monster".into());
                        }
                        "toggle_shield" => {
                            let snap = snapshot();
                            let next = !snap.system_shield;
                            let _ = set_system_shield(next);
                            if next {
                                let _ = set_system_proxy(true);
                            } else {
                                let _ = set_system_proxy(false);
                            }
                        }
                        "proxy_on" => {
                            let _ = set_system_shield(true);
                            let _ = set_system_proxy(true);
                        }
                        "proxy_off" => {
                            let _ = set_system_shield(false);
                            let _ = set_system_proxy(false);
                        }
                        "restart_host" => {
                            let _ = ensure_host();
                        }
                        "clear_log" => {
                            let _ = clear_events();
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    }
                    let _ = app.emit("tray-refresh", ());
                })
                .on_tray_icon_event(|tray, event| {
                    // Right-click still shows menu; double left opens panel as shortcut.
                    if let TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        show_window(tray.app_handle());
                    }
                    // Ignore single left — menu opens via show_menu_on_left_click.
                    let _ = MouseButtonState::Up;
                })
                .build(app)?;

            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }

            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                let mut ticks: u64 = 0;
                loop {
                    let mut snap = snapshot();
                    if !snap.connected {
                        let _ = host::ensure_running();
                        snap = snapshot();
                    }
                    let _ = tray.set_tooltip(Some(tray_title(&snap)));
                    if let Some(state) = app_handle.try_state::<Mutex<TrayMenuHandles>>() {
                        if let Ok(handles) = state.lock() {
                            refresh_tray_menu(&handles, &snap);
                        }
                    }
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
