# AGENTS.md

Guidance for AI agents and contributors working on this repository.

## Project

Unofficial, lightweight desktop wrapper for WhatsApp Web. Rust with `tao` (window/event
loop) and `wry` (system webview: WebKitGTK / WebView2 / WKWebView). No bundled Chromium.
Targets Linux, Windows and macOS; packaged as `.deb` (cargo-deb) and Snap.

- `src/main.rs`: window, webview, navigation/permission/IPC handlers, notifications.
- `src/assets/init.js`: script injected into the page (notification shim, unread title).
- `src/assets/splash.html`: loading and offline page.
- `packaging/`: `.desktop` file and icon used by the `.deb`.
- `snap/`: Snap packaging (`snapcraft.yaml`, `gui/`).
- `Makefile`: build and packaging shortcuts.

## Versioning (mandatory)

**Every change must bump `version` in `Cargo.toml`.** No exceptions, including fixes,
docs-only edits, packaging and refactors. Follow semantic versioning:

- Patch (`0.1.0` to `0.1.1`): bug fixes, docs, packaging, internal refactors.
- Minor (`0.1.x` to `0.2.0`): new user-visible features.
- Major: breaking changes (once past `1.0.0`).

When bumping the version:

1. Edit `version` in `Cargo.toml`.
2. Keep `version` in `snap/snapcraft.yaml` identical to `Cargo.toml`.
3. Run `cargo build` so `Cargo.lock` picks up the new version, and commit it with the change.
4. Make sure the bump is in the same commit as the change it describes.

The `.deb` filename comes from this version, so a missing bump produces a stale artifact.

## Build and run

```
cargo run                # debug build, devtools and diagnostic logs enabled
cargo build --release
cargo clippy             # must be clean
make deb                 # build target/debian/whatsapp-desktop_<version>-1_amd64.deb
make install-deb         # build and install with apt
```

Linux build dependencies: `libwebkit2gtk-4.1-dev libgtk-3-dev libdbus-1-dev pkg-config`.

## Security rules

This app embeds a third-party site with native bridges. Keep these invariants:

- Only `https://web.whatsapp.com` (parsed with the `url` crate, no credentials, no port)
  counts as the WhatsApp page. Never allow-list by string prefix or suffix.
- External links go through `open_external` and only `http`, `https` and `mailto` are
  passed to the OS. Never call `open::that` directly.
- Notification, microphone and camera permissions are granted only while the top-level
  page is WhatsApp (`on_wa` flag, set from the page-load handler). Everything else is denied.
- IPC messages are accepted only from the WhatsApp page.
- Any new host allowed to load in-app (for example the PDF viewer at
  `webtp.whatsapp.net/pdf-viewer/`) must match exact host and path, and must not gain
  IPC access or permissions.
- Do not add scripts that read, send or automate chat content, and do not alter
  WhatsApp's own HTML, CSS or JS. Automation risks account bans.

## Privacy

No telemetry, analytics or crash reporting. The app only talks to WhatsApp. Do not add
network calls, trackers or unnecessary extras.

## Conventions

- Commit messages use conventional prefixes (`feat:`, `fix:`, `build:`, `docs:`) and explain why.
- Run `cargo build` and `cargo clippy` before committing; both must be clean.
- Do not commit or push unless asked.
- Debug-only logging goes behind `cfg!(debug_assertions)`.
- The app icon is the WhatsApp logo (trademark of WhatsApp LLC / Meta); see the README
  before publishing to a store.
