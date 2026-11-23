//! Minimal, privacy-friendly desktop wrapper around WhatsApp Web.

use std::path::{Path, PathBuf};

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

/// Only WhatsApp's own hosts (and the local splash page) stay in-app.
fn is_internal(url: &str) -> bool {
    if url.starts_with("about:") || url.starts_with("data:") || url.starts_with("blob:") || url.starts_with("wry:") {
        return true;
    }
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#', ':']).next().unwrap_or("");
    host == "whatsapp.com" || host.ends_with(".whatsapp.com") || host.ends_with(".whatsapp.net")
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
    let builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_user_agent(USER_AGENT)
        .with_html(SPLASH)
        .with_initialization_script(INIT_JS)
        .with_clipboard(true)
        .with_devtools(cfg!(debug_assertions))
        .with_ipc_handler(move |req| handle_ipc(req.body(), &ipc_proxy))
        .with_navigation_handler(|url| {
            if is_internal(&url) {
                true
            } else {
                let _ = open::that(url);
                false
            }
        })
        .with_new_window_req_handler(|url, _| {
            if !is_internal(&url) {
                let _ = open::that(url);
            }
            NewWindowResponse::Deny
        })
        .with_permission_handler(|kind| match kind {
            PermissionKind::Notifications | PermissionKind::Microphone | PermissionKind::Camera => PermissionResponse::Allow,
            _ => PermissionResponse::Deny,
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
