# WhatsApp Desktop

Unofficial, lightweight desktop wrapper for WhatsApp Web. Rust + system webview
(WebKitGTK / WebView2 / WKWebView) via `tao` + `wry`; no bundled Chromium.
Not affiliated with WhatsApp or Meta. No telemetry; only talks to WhatsApp.

## Build
Linux deps: `sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libdbus-1-dev pkg-config`
Windows needs the WebView2 runtime (preinstalled on Win 11). macOS: nothing extra.

    cargo run --release

## Debian package
    cargo install cargo-deb
    cargo deb
    sudo apt install ./target/debian/whatsapp-desktop_*.deb

## Snap
    snapcraft
    sudo snap install --dangerous ./whatsapp-desktop_*.snap

## Behaviour
- Session persists in the OS local data dir (`whatsapp-desktop`).
- Downloads save to your Downloads folder (name collisions get a number suffix); a notification reports success or failure.
- Links to other sites open in your default browser.
- Desktop notifications via native OS notifier; unread count shows in window title.
- Paste images from the clipboard (bitmaps or image files copied in a file manager) into chats; on Linux this uses a native clipboard fallback.

## Trademark
The app icon is the WhatsApp logo (Wikimedia Commons), a trademark of WhatsApp LLC / Meta.
Used here to identify the service only; this project is unofficial and not endorsed by them.
Replace `packaging/whatsapp-desktop.svg` and `snap/gui/whatsapp-desktop.svg` before publishing
to a store such as the Snap Store, which may reject apps that imitate official branding.
