//! Minimal, privacy-friendly desktop wrapper around WhatsApp Web.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use notify_rust::Notification;
use serde_json::Value;
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::{NewWindowResponse, PermissionKind, PermissionResponse, WebContext, WebViewBuilder};

#[cfg(target_os = "linux")]
use tao::platform::unix::WindowExtUnix;
#[cfg(target_os = "linux")]
use wry::WebViewBuilderExtUnix;

const APP_NAME: &str = "WhatsApp Desktop";

// Sound played with each notification (freedesktop sound theme / Windows / macOS names).
#[cfg(target_os = "linux")]
const NOTIFY_SOUND: &str = "message-new-instant";
#[cfg(target_os = "windows")]
const NOTIFY_SOUND: &str = "IM";
#[cfg(target_os = "macos")]
const NOTIFY_SOUND: &str = "Ping";
const SPLASH: &str = include_str!("assets/splash.html");
const INIT_JS: &str = include_str!("assets/init.js");

// WhatsApp Web rejects unknown browsers; present as current desktop Chrome.
#[cfg(target_os = "linux")]
const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
#[cfg(target_os = "windows")]
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36";
#[cfg(target_os = "macos")]
const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Safari/605.1.15";

enum UserEvent {
    Title(String),
}

const WA_HOSTS: [&str; 1] = ["web.whatsapp.com"];

/// True for https pages on WhatsApp's own web host (no credentials in URL).
fn is_whatsapp_page(url: &str) -> bool {
    let Ok(u) = url::Url::parse(url) else { return false };
    u.scheme() == "https"
        && u.username().is_empty()
        && u.password().is_none()
        && u.port().is_none()
        && u.host_str().is_some_and(|h| WA_HOSTS.contains(&h))
}

/// WhatsApp's embedded PDF viewer frame. Allowed to load in-app, but it does
/// not get the permissions or IPC access of the main page.
fn is_pdf_viewer(url: &str) -> bool {
    let Ok(u) = url::Url::parse(url) else { return false };
    u.scheme() == "https"
        && u.username().is_empty()
        && u.password().is_none()
        && u.port().is_none()
        && u.host_str() == Some("webtp.whatsapp.net")
        && u.path().starts_with("/pdf-viewer/")
}

/// Navigations that stay inside the app. `first` allows the initial splash
/// page (loaded as data:/about: depending on platform) exactly once.
fn is_internal(url: &str, first: &Cell<bool>) -> bool {
    if first.replace(false) {
        return true;
    }
    if url == "about:blank" || is_whatsapp_page(url) || is_pdf_viewer(url) {
        return true;
    }
    // blob: URLs created by WhatsApp itself (media/downloads).
    url::Url::parse(url).is_ok_and(|u| u.scheme() == "blob" && is_whatsapp_page(u.path()))
}

/// Hand a link to the OS only if it is a plain web or mail link.
/// Blocks file://, smb://, custom app schemes, etc.
fn open_external(url: &str) {
    let lower = url.to_ascii_lowercase();
    let allowed = lower.starts_with("https://") || lower.starts_with("http://") || lower.starts_with("mailto:");
    if allowed && !url.chars().any(char::is_control) {
        let _ = open::that(url);
    }
}

/// Pick a non-clobbering path in the user's Downloads folder.
fn download_path(suggested: &Path) -> PathBuf {
    let dir = dirs::download_dir().unwrap_or_else(std::env::temp_dir);
    let name = suggested
        .file_name()
        .map(|n| n.to_owned())
        .unwrap_or_else(|| "download".into());
    let mut out = dir.join(&name);
    let stem = Path::new(&name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = Path::new(&name).extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let mut n = 1;
    while out.exists() {
        out = dir.join(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    out
}

fn handle_ipc(body: &str, proxy: &tao::event_loop::EventLoopProxy<UserEvent>) {
    let Ok(v) = serde_json::from_str::<Value>(body) else { return };
    let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_owned();
    match v.get("t").and_then(Value::as_str) {
        Some("notify") => {
            if cfg!(debug_assertions) {
                eprintln!("notify: {} / {}", text("title"), text("body"));
            }
            if let Err(e) = Notification::new()
                .appname(APP_NAME)
                .icon("whatsapp-desktop")
                .sound_name(NOTIFY_SOUND)
                .hint(notify_rust::Hint::DesktopEntry("whatsapp-desktop".into()))
                .summary(&text("title"))
                .body(&text("body"))
                .show()
            {
                eprintln!("notification failed: {e}");
            }
        }
        Some("title") => {
            let _ = proxy.send_event(UserEvent::Title(text("title")));
        }
        _ => {}
    }
}

#[cfg(target_os = "macos")]
fn install_macos_menu() {
    use muda::{Menu, PredefinedMenuItem, Submenu};
    let menu = Menu::new();
    let app = Submenu::new(APP_NAME, true);
    let _ = app.append_items(&[&PredefinedMenuItem::hide(None), &PredefinedMenuItem::separator(), &PredefinedMenuItem::quit(None)]);
    let edit = Submenu::new("Edit", true);
    let _ = edit.append_items(&[
        &PredefinedMenuItem::undo(None),
        &PredefinedMenuItem::redo(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::cut(None),
        &PredefinedMenuItem::copy(None),
        &PredefinedMenuItem::paste(None),
        &PredefinedMenuItem::select_all(None),
    ]);
    let _ = menu.append_items(&[&app, &edit]);
    menu.init_for_nsapp();
    std::mem::forget(menu);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title(APP_NAME)
        .with_inner_size(LogicalSize::new(1100.0, 760.0))
        .with_min_inner_size(LogicalSize::new(800.0, 560.0))
        .build(&event_loop)?;

    #[cfg(target_os = "macos")]
    install_macos_menu();

    // Persistent profile so the QR login survives restarts.
    let data_dir = dirs::data_local_dir().map(|d| d.join("whatsapp-desktop"));
    let mut context = WebContext::new(data_dir);

    let ipc_proxy = proxy.clone();
    let first_nav = Rc::new(Cell::new(true));
    let nav_first = first_nav.clone();
    let win_first = first_nav.clone();
    // Set only while the top-level page is WhatsApp; gates device permissions.
    let on_wa = Arc::new(AtomicBool::new(false));
    let load_on_wa = on_wa.clone();
    let perm_on_wa = on_wa.clone();
    let builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_user_agent(USER_AGENT)
        .with_html(SPLASH)
        .with_initialization_script(INIT_JS)
        .with_clipboard(true)
        .with_devtools(cfg!(debug_assertions))
        .with_ipc_handler(move |req| {
            // Only WhatsApp's own page may talk to the native side.
            if is_whatsapp_page(&req.uri().to_string()) {
                handle_ipc(req.body(), &ipc_proxy);
            }
        })
        // Page-load events report the top-level URL: fail closed on anything
        // that is not the WhatsApp page (including a top-level pdf-viewer).
        .with_on_page_load_handler(move |_event, url| {
            load_on_wa.store(is_whatsapp_page(&url), Ordering::Relaxed);
        })
        .with_navigation_handler(move |url| {
            if is_internal(&url, &nav_first) {
                true
            } else {
                if cfg!(debug_assertions) {
                    eprintln!("blocked navigation: {url}");
                }
                open_external(&url);
                false
            }
        })
        .with_new_window_req_handler(move |url, _| {
            if cfg!(debug_assertions) {
                eprintln!("new window request: {url}");
            }
            if !is_internal(&url, &win_first) {
                open_external(&url);
            }
            NewWindowResponse::Deny
        })
        .with_permission_handler(move |kind| match kind {
            PermissionKind::Notifications | PermissionKind::Microphone | PermissionKind::Camera
                if perm_on_wa.load(Ordering::Relaxed) =>
            {
                PermissionResponse::Allow
            }
            other => {
                if cfg!(debug_assertions) {
                    eprintln!("permission denied: {other:?}");
                }
                PermissionResponse::Deny
            }
        })
        .with_download_started_handler(|_url, path| {
            *path = download_path(path);
            true
        });

    #[cfg(not(target_os = "linux"))]
    let webview = builder.build(&window)?;
    #[cfg(target_os = "linux")]
    let webview = builder.build_gtk(window.default_vbox().ok_or("no gtk vbox")?)?;
    let _keep = &webview;

    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => *flow = ControlFlow::Exit,
            Event::UserEvent(UserEvent::Title(t)) => {
                let unread = t.trim().starts_with('(');
                window.set_title(&if unread { format!("{t} - {APP_NAME}") } else { APP_NAME.to_string() });
            }
            _ => {}
        }
    });
}
