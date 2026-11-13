# WhaRa

**The original WhatsApp Web, in a lightweight native macOS window.**  
Rust · Tauri 2 · WKWebView · macOS 12+

WhaRa keeps the familiar WhatsApp Web experience and adds a macOS-friendly shell, a compact single-pane layout, and a few convenient UI controls. It is built for people who want to use WhatsApp Web in its own desktop window without Electron.

| Compact chat list | Compact conversation |
| :---: | :---: |
| <img src="docs/screenshots/compact-chat-list-demo.png" alt="WhaRa compact chat list with synthetic contacts, chat previews and unread counts" width="300"> | <img src="docs/screenshots/compact-stella-demo.png" alt="WhaRa compact Stella (You) conversation with synthetic demo messages" width="300"> |

<p align="center"><sub>Compact mode (470 × 740 window). Both screenshots are staged illustrative previews. <strong>All names, avatars, messages, timestamps and unread counts are synthetic</strong> — no private conversations or contacts are published.</sub></p>

## Download for macOS

**[Download the latest WhaRa release](https://github.com/rayenking/WhaRa/releases/latest)** — no Rust toolchain or manual build required.

Choose the installer that matches your Mac:

- **Apple Silicon (M1/M2/M3/M4/...)**: `WhaRa_vX.Y.Z_apple-silicon.dmg`
- **Intel Mac**: `WhaRa_vX.Y.Z_intel.dmg`

Open the downloaded DMG and drag **WhaRa.app** into Applications. A zipped `.app` is also provided for each architecture, along with SHA-256 checksums. **From WhaRa v0.2.0 onward, later updates can be installed directly in the app**; v0.1.0 needs a one-time manual installation of a v0.2.0-or-newer release.

**macOS security notice:** Current downloads use **ad-hoc signatures, not Apple Developer ID notarization**. macOS Gatekeeper may ask you to approve opening the app in **System Settings → Privacy & Security**. Only download from the repository's official Releases page and verify the checksum if desired.

## Original WhatsApp Web — not an unofficial WhatsApp library

**WhaRa loads [web.whatsapp.com](https://web.whatsapp.com/) directly in macOS WKWebView.** WhatsApp Web itself handles login, QR linking, conversations, media, and supported calling features.

- **No unofficial WhatsApp client library**, reverse-engineered messaging protocol, or alternative WhatsApp backend.
- **No reimplementation of the chat engine.** WhaRa applies scoped, reversible JavaScript/CSS UI enhancements to the WhatsApp Web page and bridges certain desktop features to native Rust/Tauri functionality.
- Native WhatsApp controls still perform the actual WhatsApp actions. For example, the combined Voice/Video menu forwards clicks to WhatsApp's own call buttons.

This is an **independent third-party macOS wrapper**, not an official WhatsApp or Meta application, and is not affiliated with or endorsed by either company.

## Features

### Compact / mobile-like layout

- **Responsive single-pane mode** below 800 CSS pixels: the chat list and the selected conversation take turns occupying the available space.
- **470 × 740 minimum window size**, inspired by Telegram's compact window dimensions.
- **Back-to-chats navigation** when viewing a conversation in compact mode.
- **Compact Contact / Group info** that overlays the conversation instead of breaking the layout into oversized desktop panes.
- **Media, Links & Docs** navigation inside the compact details view, plus support for WhatsApp's native photo viewer.
- **Better chat width and message spacing** for narrow windows; full desktop layout remains available at larger sizes.

### WhatsApp UI conveniences

- **Combined call menu** with the original WhatsApp Voice and Video icons and their native actions.
- **Add to list** shortcut moved to the sidebar for compact navigation, with the native list menu opening alongside it.
- **Original WhatsApp Media sidebar action** retained.
- Changes are restricted to the UI layer; WhatsApp's message list and native actions remain owned by WhatsApp Web.

### macOS integrations

- **Native WKWebView** shell using Rust and Tauri 2 — no Electron.
- **Verified in-app updates (v0.2.0+)**: automatic checks after launch and every six hours; a native **Update / Later** popup; download progress, signature verification, installation and automatic app restart.
- Choosing **Later** postpones the prompt for roughly 24 hours while allowing newer versions to appear immediately. Manual checks are available in Preferences.
- **Menu-bar / tray access** with an unread counter and quick actions.
- **Native notifications** bridged from WhatsApp Web to macOS Notification Center.
- **Privacy veil** that can blur chat contents when the window is not focused, with a configurable global shortcut (default: Cmd+Shift+B).
- **Native WebView zoom** from 0.6× to 2×, avoiding CSS zoom interference with WhatsApp's layouts.
- **Launch at login**, **single instance**, **window geometry restoration**, and **keep running in the menu bar** when the chat window is closed.
- **Preferences window** with auto-saved settings.

## Getting started

**Requirements:** macOS 12+, a recent stable Rust toolchain (Rust 1.82 or newer), and Tauri CLI v2. You'll link your WhatsApp account through WhatsApp Web's own login flow.

Install the Tauri CLI if you don't already have it:

~~~bash
cargo install tauri-cli --version "^2" --locked
~~~

Run from the repository root:

~~~bash
cargo tauri dev --no-watch --config src-tauri/tauri.conf.json
~~~

Or build a native executable:

~~~bash
cargo build --release --manifest-path src-tauri/Cargo.toml
./target/release/whara
~~~

To create a macOS app bundle on your own machine:

~~~bash
cargo tauri build --config src-tauri/tauri.conf.json
~~~

## CI/CD and releases

GitHub Actions runs formatting checks, the Rust test suite and injected-JavaScript syntax checks on updates to `main` and pull requests. When a `vX.Y.Z` Git tag is pushed, the release workflow:

1. Confirms the Git tag matches the Cargo and Tauri versions and runs tests.
2. Builds macOS DMGs and ZIPs for **Apple Silicon** and **Intel**.
3. Creates a signed `.app.tar.gz` updater archive and `.sig` for each architecture.
4. Generates `latest.json` pointing to both signed updater archives, plus a `SHA256SUMS.txt` checksum file.
5. Publishes the installers and signed updates in [GitHub Releases](https://github.com/rayenking/WhaRa/releases), **only after both architectures pass**.

### Maintainer: secure update signing

Tauri updater archives use a separate **minisign signature** in addition to the ad-hoc macOS app signature. The public verification key is embedded in `src-tauri/tauri.conf.json`. The private signing key must **never** be included in Git commits or downloadable artifacts.

For this project, the original private key was generated locally at `~/.config/whara-updater/release.key`. Before creating the first signed GitHub release, the repository owner must add the contents of that file as a GitHub Actions repository secret named **`TAURI_SIGNING_PRIVATE_KEY`** (Settings → Secrets and variables → Actions). Keep a secure backup: losing the key prevents installed versions from trusting future updates. Do not generate a replacement key casually.

Once the secret is configured, bump the versions in `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`, push changes, and push a matching `vX.Y.Z` tag. The workflow builds signed archives and a static updater manifest automatically.

The app bundle still uses ad-hoc macOS code signing, so a first install may require Gatekeeper confirmation. Update signatures prevent WhaRa from silently installing unsigned or improperly signed application updates.

## How it works

~~~text
WhaRa (Rust / Tauri)
└── macOS WKWebView ── https://web.whatsapp.com/
    ├── WhatsApp Web's own chat, media, login, and controls
    ├── whara-inject.js  — native notifications, unread title, privacy veil
    └── whara-ui.js      — reversible compact layout and UI proxies
~~~

The JavaScript injected by WhaRa runs inside the loaded WhatsApp Web document. The native shell provides window management, preferences, tray integration, zoom, notifications, and startup behavior. **It does not replace WhatsApp Web's transport or messaging implementation.**

Most UI customization can be disabled independently at runtime for debugging:

~~~js
__wharaUi.setEnabled('compact', false);
__wharaUi.setEnabled('lists', false);
__wharaUi.setEnabled('calls', false);
// __wharaUi.destroy(); // remove the injected UI layer
~~~

These flags reset when the app restarts.

### Project layout

~~~text
prefs/index.html                   Native settings interface
prefs/update.html                  Local Update / Later popup and progress UI
scripts/make-icons.py              App / tray icon generator
scripts/make-updater-manifest.py    GitHub Releases signed updater manifest
src-tauri/src/                     Rust shell, signed updater, tray, preferences
src-tauri/resources/whara-inject.js Notification / privacy bridge
src-tauri/resources/whara-ui.js     Compact layout + WhatsApp UI proxies
src-tauri/capabilities/            Tauri permissions
src-tauri/icons/                   App and tray icons
docs/screenshots/                  Privacy-safe demonstration screenshots
~~~

## Tests

~~~bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
~~~

The injected JavaScript can also be checked with a JavaScript parser. Runtime UI behavior should be verified on macOS because WhatsApp Web updates its private DOM independently of this project.

## Limitations and privacy

- WhatsApp Web is a changing third-party website. Internal DOM selectors and layout behavior can change without notice, so compact UI enhancements may need maintenance.
- Calling and other WhatsApp features depend on what the underlying WhatsApp Web session and WKWebView support.
- WhaRa is **not** a separate WhatsApp service and does not provide an unofficial messaging API.
- Both README screenshots are **staged, illustrative UI previews**; the compact list uses synthetic contacts and the *Stella (You)* conversation uses synthetic messages. Neither contains actual account conversations or contacts.

## License

MIT — see [LICENSE](LICENSE).
