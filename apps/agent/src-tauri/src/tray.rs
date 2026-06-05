//! System-tray surface (v2 plan §4.1 + §11.1). The tray is the *primary* UX —
//! every routine action is reachable here without opening the panel window:
//!
//! ```text
//!  HWAX Agent · <ver>
//!  ─────────────────
//!  ▶ <module>  <ver>  (submenu: 실행/업데이트/상세)   ← built from list_modules
//!  ─────────────────
//!  모두 업데이트 확인 / 지금 동기화 / 로그 폴더 열기 / 설정...
//!  ─────────────────
//!  페어링 다시 하기 / 종료
//! ```
//!
//! Left-click toggles the main panel window; the colored status dot (green/
//! yellow/red) is reflected in the tooltip (and, where icon assets exist, the
//! tray icon — see the note in [`refresh`]).

use crate::state::{AppState, StatusColor};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

/// Stable id of the tray icon, so we can fetch it back to update tooltip/icon.
pub const TRAY_ID: &str = "hwax-tray";

// Menu item ids (matched in `handle_menu_event`). Module actions are encoded as
// `mod:<action>:<id>` so the handler can route them generically.
const ID_SYNC: &str = "sync";
const ID_CHECK_ALL: &str = "check_all";
const ID_LOGS: &str = "logs";
const ID_SETTINGS: &str = "settings";
const ID_REPAIR: &str = "repair";
const ID_QUIT: &str = "quit";

/// Build the tray at startup. The module submenu is (re)built by [`refresh`].
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let icon = app
        .default_window_icon()
        .cloned()
        .expect("bundled default window icon (icons/icon.ico) is required");

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .menu(&menu)
        .tooltip(format!("HWAX Agent · {}", crate::state::AGENT_VERSION))
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            // Left-click toggles the panel window (v2 §4.1).
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("main") {
                    let visible = win.is_visible().unwrap_or(false);
                    if visible {
                        let _ = win.hide();
                    } else {
                        let _ = win.show();
                        let _ = win.set_focus();
                    }
                }
            }
        })
        .build(app)?;
    Ok(())
}

/// Assemble the full tray menu. The module submenu is populated from the cached
/// manifest + local state via [`crate::sync::build_module_views`].
fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let modules = Submenu::with_id(app, "modules", "프로그램", true)?;
    if let Some(state) = app.try_state::<AppState>() {
        for m in crate::sync::build_module_views(&state) {
            let label = match (&m.current_version, &m.latest_version) {
                (Some(cur), _) => format!("{}  {}", m.name, cur),
                (None, Some(lat)) => format!("{}  {} (설치)", m.name, lat),
                _ => m.name.clone(),
            };
            // A small submenu per module: run / update / details.
            let sub = Submenu::with_id(app, format!("modsub:{}", m.id), label, true)?;
            sub.append(&MenuItem::with_id(
                app,
                format!("mod:run:{}", m.id),
                "실행",
                true,
                None::<&str>,
            )?)?;
            sub.append(&MenuItem::with_id(
                app,
                format!("mod:update:{}", m.id),
                "업데이트/설치",
                true,
                None::<&str>,
            )?)?;
            sub.append(&MenuItem::with_id(
                app,
                format!("mod:detail:{}", m.id),
                "상세...",
                true,
                None::<&str>,
            )?)?;
            modules.append(&sub)?;
        }
    }

    let header = MenuItem::with_id(
        app,
        "header",
        format!("HWAX Agent · {}", crate::state::AGENT_VERSION),
        false, // disabled: it's a label
        None::<&str>,
    )?;
    let check_all = MenuItem::with_id(app, ID_CHECK_ALL, "모두 업데이트 확인", true, None::<&str>)?;
    let sync = MenuItem::with_id(app, ID_SYNC, "지금 동기화", true, None::<&str>)?;
    let logs = MenuItem::with_id(app, ID_LOGS, "로그 폴더 열기", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, ID_SETTINGS, "설정...", true, None::<&str>)?;
    let repair = MenuItem::with_id(app, ID_REPAIR, "페어링 다시 하기", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, ID_QUIT, "종료", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &header,
            &PredefinedMenuItem::separator(app)?,
            &modules,
            &PredefinedMenuItem::separator(app)?,
            &check_all,
            &sync,
            &logs,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &repair,
            &quit,
        ],
    )
}

/// Rebuild the tray menu (after a sync changes the module list) and refresh the
/// tooltip to reflect the current status color + last-sync time.
pub fn refresh(app: &AppHandle) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    if let Ok(menu) = build_menu(app) {
        let _ = tray.set_menu(Some(menu));
    }
    if let Some(state) = app.try_state::<AppState>() {
        let color = state.status_color();
        let dot = match color {
            StatusColor::Green => "정상",
            StatusColor::Yellow => "경고",
            StatusColor::Red => "에러",
        };
        let sync = state
            .last_sync()
            .map(|t| format!(" · 마지막 동기화 {t}"))
            .unwrap_or_default();
        let _ = tray.set_tooltip(Some(format!(
            "HWAX Agent · {} · {dot}{sync}",
            crate::state::AGENT_VERSION
        )));
        // NOTE: status-colored icons (icon_green/yellow/red.ico) are bundled by
        // the build pipeline; once present, swap here via `tray.set_icon(...)`.
        // We avoid inventing fake binary icon files in source control.
    }
}

/// Route a tray menu click. Module actions (`mod:<action>:<id>`) are dispatched
/// to the relevant command; the fixed items map to sync/logs/settings/etc.
fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let id = event.id().0.as_str().to_string();
    let app = app.clone();

    if let Some(rest) = id.strip_prefix("mod:") {
        // rest = "<action>:<module_id>"
        if let Some((action, module_id)) = rest.split_once(':') {
            let action = action.to_string();
            let module_id = module_id.to_string();
            tauri::async_runtime::spawn(async move {
                crate::commands::dispatch_tray_module_action(&app, &action, &module_id).await;
            });
        }
        return;
    }

    match id.as_str() {
        ID_SYNC | ID_CHECK_ALL => {
            tauri::async_runtime::spawn(async move {
                if crate::sync::sync_now(&app).await.is_ok() {
                    refresh(&app);
                }
            });
        }
        ID_LOGS => {
            tauri::async_runtime::spawn(async move {
                let _ = crate::commands::open_logs_folder(&app);
            });
        }
        ID_SETTINGS => show_main(&app),
        ID_REPAIR => {
            // Show the panel; the React "re-pair" flow drives start_pairing.
            show_main(&app);
        }
        ID_QUIT => {
            app.exit(0);
        }
        _ => {}
    }
}

/// Show + focus the main panel window.
fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}
