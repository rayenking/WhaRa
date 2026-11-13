# WhaRa local UI development (no Web Inspector)

WhaRa's debug build includes a small **local-only command bridge** for inspecting and
modifying the running WKWebView. It exists to debug WhatsApp Web's changing
layout without opening Developer Tools, clicking the desktop, or relying on
AppleScript/Accessibility UI selectors.

It is compiled **only in debug builds on macOS**. Release builds do not
start a socket or include the bridge module.

## Start

From the repository root:

```sh
cargo build --manifest-path src-tauri/Cargo.toml
./target/debug/whara
```

If another WhaRa instance is running, close it normally first (WhaRa is
single-instance). The development socket lives at:

```
~/Library/Caches/whara-dev-bridge/bridge.sock
```

The bridge directory is mode `0700` and its socket mode `0600`. Only local
processes running as the current macOS user can connect. There is **no TCP port**,
public API, or new WhatsApp-facing Tauri IPC command.

## Useful commands

All commands run from the repository root:

```sh
# Is the debug application connected?
python3 scripts/whara-dev.py status

# Safe structure-only report: viewport, panes, drawers, widths, scroll,
# sidebar icon availability (no contact names or message contents).
python3 scripts/whara-dev.py inspect

# Resize the app without using a mouse.
python3 scripts/whara-dev.py size 470 800
python3 scripts/whara-dev.py size 980 800

# Navigate through WhatsApp's *original* sidebar button handlers and inspect.
python3 scripts/whara-dev.py nav Calls
python3 scripts/whara-dev.py nav Status
python3 scripts/whara-dev.py nav Channels
python3 scripts/whara-dev.py nav Communities
python3 scripts/whara-dev.py nav Media
python3 scripts/whara-dev.py nav Settings
python3 scripts/whara-dev.py nav Chats

# Evaluate a synchronous JS expression and receive its JSON result.
python3 scripts/whara-dev.py eval '({mode:__wharaUi?.diagnostics().compactMode, width:innerWidth})'

# Execute a script file in the existing WKWebView.
# To return a value, end your JavaScript file with a return statement.
python3 scripts/whara-dev.py run /tmp/inspect-whara.js

# Trial CSS: no source changes, immediately visible in the current view.
python3 scripts/whara-dev.py css /tmp/compact-fix.css
python3 scripts/whara-dev.py clear-css

# After editing src-tauri/resources/whara-ui.js:
# Destroys the old injected WhaRa controls/styles and starts the new script,
# while leaving WhatsApp Web, its React nodes and account session in place.
python3 scripts/whara-dev.py reload-ui
```

`eval` accepts a **single synchronous JavaScript expression**. The `run`
subcommand accepts JavaScript statements (with explicit `return` for output).
The bridge inserts the code directly through native WKWebView's JavaScript
execution API rather than using JS `eval()`/`new Function()`, which WhatsApp
blocks via Content Security Policy.

Example `/tmp/inspect-whara.js`:

```js
const drawer = document.querySelector('[data-testid="drawer-left"]');
const rect = drawer?.getBoundingClientRect();
return {
  hasDrawer: Boolean(drawer),
  drawerWidth: rect && Math.round(rect.width),
  viewportWidth: innerWidth
};
```

## Privacy and security

**Only run scripts you wrote or trust.** The development tool can evaluate
arbitrary JavaScript in a logged-in WhatsApp Web page and could read private
conversations if instructed to. The built-in `inspect` command intentionally
reports only layout geometry and known navigation labels.

Do not print or publish message bodies, phone numbers, contact names, cookies,
QR tokens or session credentials from a real account. If screenshots are
required for public documentation, use synthetic fixtures and do not commit a
debug overlay into production injection.

The debug socket is local and private, but another process running as your
macOS user could connect. Quit the debug app when not developing. Never expose
the socket to a remote shell, port-forward or untrusted automation.

## About releases

- `cargo build --release` / `cargo tauri build` excludes `dev_bridge.rs`.
- No Web Inspector is opened automatically, even in a debug build.
- Development-only CSS previews are not persisted and disappear when you reload.
- Your existing release updater and WhatsApp Web networking are unaffected.
