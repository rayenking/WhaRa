#!/usr/bin/env python3
"""Control a running DEBUG WhaRa WebView without Web Inspector or screen clicks.

Commands use a local AF_UNIX socket (0700 directory, 0600 socket) and return
results straight from WebKit. Only run trusted scripts: JavaScript can inspect
the active WhatsApp Web session, including private messages if you ask it to.

No debug socket or JS evaluator is compiled into production release builds.
"""

import argparse
import json
from pathlib import Path
import socket
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
SOCKET = Path.home() / "Library" / "Caches" / "whara-dev-bridge" / "bridge.sock"
MAX_BYTES = 512 * 1024

# No chat content, contact names, message text, phone numbers or avatars.
SAFE_INSPECT = r"""(() => {
  const box = (element) => {
    if (!element) return null;
    const r = element.getBoundingClientRect();
    const style = getComputedStyle(element);
    return {
      x: Math.round(r.x), y: Math.round(r.y),
      width: Math.round(r.width), height: Math.round(r.height),
      display: style.display, overflowX: style.overflowX,
      scrollWidth: element.scrollWidth, clientWidth: element.clientWidth
    };
  };
  const root = document.documentElement;
  const drawer = document.querySelector('#app [data-testid="drawer-fullscreen"]');
  return {
    viewport: {width: innerWidth, height: innerHeight, devicePixelRatio},
    mode: globalThis.__wharaUi?.diagnostics?.().compactMode ?? 'native',
    document: {scrollWidth: root.scrollWidth, clientWidth: root.clientWidth},
    two: box(document.querySelector('#app .two')),
    sidebar: box(document.querySelector('#side')),
    main: box(document.querySelector('#main')),
    drawer: box(drawer),
    drawerLeft: box(document.querySelector('#app [data-testid="drawer-left"]')),
    drawerMiddle: box(document.querySelector('#app [data-testid="drawer-middle"]')),
    drawerRight: box(document.querySelector('#app [data-testid="drawer-right"]')),
    sections: Object.fromEntries(
      ['settings-drawer', 'calls-list', 'status-list', 'media-gallery-drawer',
        'pane-side', 'drawer-fullscreen', 'drawer-left', 'drawer-middle', 'drawer-right']
        .map(name => [name, box(document.querySelector('#app [data-testid="' + name + '"]'))])
    ),
    navigation: ['Chats','Calls','Status','Channels','Communities','Media','Settings']
      .filter(name => !!document.querySelector('button[aria-label="' + name + '"]')),
    devUi: !!globalThis.__wharaUi
  };
})()"""

NAV_NAMES = ("Chats", "Calls", "Status", "Channels",
             "Communities", "Media", "Settings")


def send(payload: dict, socket_file: Path, timeout: float = 15.0) -> dict:
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as conn:
            conn.settimeout(timeout)
            conn.connect(str(socket_file))
            data = (json.dumps(payload, ensure_ascii=False) + "\n").encode()
            if len(data) > MAX_BYTES:
                raise ValueError("Script larger than 512 KiB; split it into smaller evaluations")
            conn.sendall(data)
            response = bytearray()
            while b"\n" not in response:
                chunk = conn.recv(65536)
                if not chunk:
                    break
                response.extend(chunk)
                if len(response) > MAX_BYTES:
                    raise ValueError("Server response exceeds 512 KiB; narrow the DOM query")
            if not response:
                raise RuntimeError("Debug bridge closed without a response")
            return json.loads(response.partition(b"\n")[0])
    except (FileNotFoundError, ConnectionRefusedError, TimeoutError) as exc:
        raise RuntimeError(
            f"Debug WhaRa bridge unavailable ({socket_file}). "
            "Run: cargo build --manifest-path src-tauri/Cargo.toml && "
            "./target/debug/whara"
        ) from exc


def js_eval(source: str, socket_file: Path) -> dict:
    return send({"op": "eval", "code": source}, socket_file)


def js_exec(source: str, socket_file: Path) -> dict:
    return send({"op": "exec", "code": source}, socket_file)


def output(value: dict) -> None:
    print(json.dumps(value, ensure_ascii=False, indent=2))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--socket", type=Path, default=SOCKET, help="Alternate bridge socket")
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("status", help="Test the local debug bridge")
    sub.add_parser("inspect", help="Privacy-safe DOM and layout geometry")
    sub.add_parser("reload-ui", help="Hot-reload src-tauri/resources/whara-ui.js")
    sub.add_parser("clear-css", help="Remove preview CSS without restarting")
    eval_cmd = sub.add_parser("eval", help="Evaluate synchronous JavaScript")
    eval_cmd.add_argument("javascript")
    run_cmd = sub.add_parser("run", help="Run a local JavaScript file")
    run_cmd.add_argument("path", type=Path)
    css_cmd = sub.add_parser("css", help="Preview a CSS file (not saved into the app)")
    css_cmd.add_argument("path", type=Path)
    nav_cmd = sub.add_parser("nav", help="Click an ORIGINAL WhatsApp sidebar button")
    nav_cmd.add_argument("section", choices=NAV_NAMES)
    nav_cmd.add_argument("--no-inspect", action="store_true")
    size_cmd = sub.add_parser("size", help="Resize the WhaRa window without GUI automation")
    size_cmd.add_argument("width", type=float)
    size_cmd.add_argument("height", type=float)
    args = parser.parse_args()
    try:
        if args.command == "status":
            result = send({"op": "ping"}, args.socket)
        elif args.command == "inspect":
            result = js_eval(SAFE_INSPECT, args.socket)
        elif args.command == "eval":
            result = js_eval(args.javascript, args.socket)
        elif args.command == "run":
            result = js_exec(args.path.read_text(encoding="utf-8"), args.socket)
        elif args.command == "reload-ui":
            script = (ROOT / "src-tauri/resources/whara-ui.js").read_text()
            # Do not reload the whole WhatsApp page or disturb its session.
            code = "globalThis.__wharaUi?.destroy?.();\n" + script + "\nreturn !!globalThis.__wharaUi;"
            result = js_exec(code, args.socket)
        elif args.command == "css":
            css = args.path.read_text(encoding="utf-8")
            code = (
                "(() => { const id = 'whara-dev-preview-css'; "
                "let s = document.getElementById(id); "
                "if (!s) { s = document.createElement('style'); "
                "s.id = id; document.head.append(s); } "
                f"s.textContent = {json.dumps(css)}; "
                "return {applied:true, bytes:s.textContent.length}; })()"
            )
            result = js_eval(code, args.socket)
        elif args.command == "clear-css":
            result = js_eval(
                "(() => { document.getElementById('whara-dev-preview-css')?.remove(); "
                "return {cleared:true}; })()", args.socket
            )
        elif args.command == "nav":
            # This invokes WhatsApp's own click handler; no React nodes moved.
            label = json.dumps(args.section)
            result = js_eval(
                "(() => { const label = " + label + "; "
                "const b = document.querySelector('button[aria-label=' "
                "+ JSON.stringify(label) + ']'); "
                "if (!b) return {clicked:false, error:'Navigation button missing'}; "
                "b.click(); return {clicked:true, section:label}; })()",
                args.socket,
            )
            if result.get("ok") and result.get("result", {}).get("clicked") and not args.no_inspect:
                time.sleep(0.6)
                result = {"navigation": result, "layout": js_eval(SAFE_INSPECT, args.socket)}
        elif args.command == "size":
            result = send({"op": "size", "width": args.width, "height": args.height}, args.socket)
        else:
            parser.error("Unknown operation")
            return 2
        output(result)
        if result.get("ok") is False:
            return 1
        return 0
    except (OSError, RuntimeError, ValueError) as exc:
        print(f"whara-dev: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
