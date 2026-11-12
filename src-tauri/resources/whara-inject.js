// WhaRa page layer — injected into the WhatsApp Web document.
// Talks back to the Rust host through the Tauri bridge and provides the
// hooks used by our own features: unread badge, privacy veil and zoom.
(() => {
  const host = location.hostname;
  if (!/(^|\.)whatsapp\.com$/.test(host)) return;
  if (globalThis.__whara) return;

  // Resolved lazily: an initialization script runs before the host installs
  // `window.__TAURI__`, so reading it at the top level captures undefined for
  // the lifetime of the page.
  const call = (name, payload) => {
    try {
      const invoke = globalThis.__TAURI__?.core?.invoke;
      if (typeof invoke === "function") return invoke(name, payload ?? {});
    } catch {
      /* host bridge not ready yet */
    }
    return Promise.resolve(null);
  };

  // WebKit exposes the Notifications API but has no permission UI on macOS:
  // `requestPermission()` settles on "denied" straight away, which is why
  // WhatsApp's own "Turn on" button snaps shut the moment it is pressed.
  // Answer from our layer instead and let the host post a real notification
  // through the notification centre.
  const announce = (title, body) => {
    const head = String(title ?? "");
    const detail = String(body ?? "");
    if (!head && !detail) return;
    call("push_notice", { title: head, body: detail });
  };

  const installNotice = () => {
    const Native = globalThis.Notification;
    if (typeof Native !== "function" || Native.__whara) return;

    // Called with and without `new`; both shapes have to work.
    const Shim = function Notice(title, options) {
      const notice = this instanceof Shim ? this : Object.create(Shim.prototype);
      announce(title, options?.body ?? "");
      notice.title = String(title ?? "");
      notice.body = String(options?.body ?? "");
      notice.tag = options?.tag ?? "";
      notice.data = options?.data ?? null;
      notice.onclick = null;
      notice.onclose = null;
      notice.onerror = null;
      notice.onshow = null;
      notice.close = () => {};
      notice.addEventListener = () => {};
      notice.removeEventListener = () => {};
      return notice;
    };

    Object.defineProperty(Shim, "permission", {
      configurable: true,
      get: () => "granted",
    });
    Shim.requestPermission = () => Promise.resolve("granted");
    Shim.maxActions = Native.maxActions ?? 0;
    Shim.__whara = true;
    globalThis.Notification = Shim;

    // WhatsApp reads the state through the Permissions API as well, and WebKit
    // answers "prompt" there no matter what the shim reports. Without this the
    // notification help panel still appears and offers a button that can only
    // open a browser preference pane that does not exist inside the panel.
    try {
      const permissions = navigator.permissions;
      if (permissions && !permissions.__whara) {
        const query = permissions.query.bind(permissions);
        permissions.query = (descriptor) => {
          if (descriptor?.name === "notifications") {
            return Promise.resolve({
              state: "granted",
              name: "notifications",
              onchange: null,
              addEventListener: () => {},
              removeEventListener: () => {},
              dispatchEvent: () => false,
            });
          }
          return query(descriptor);
        };
        permissions.__whara = true;
      }
    } catch {
      /* permissions API unavailable */
    }

    // Some call sites go through the service-worker registration instead.
    const worker = globalThis.ServiceWorkerRegistration;
    if (worker?.prototype?.showNotification && !worker.prototype.showNotification.__whara) {
      const replaced = (title, options) => {
        announce(title, options?.body ?? "");
        return Promise.resolve();
      };
      replaced.__whara = true;
      worker.prototype.showNotification = replaced;
    }
  };

  installNotice();
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", installNotice, { once: true });
  }

  const memory = { enabled: false, pinned: false, badge: -1 };
  let sheet = null;
  let ticker = 0;

  // Responsive/layout DOM injection is intentionally disabled. WhatsApp native layout remains untouched.

  const mountSheet = () => {
    if (sheet?.isConnected) return;
    sheet = document.createElement("style");
    sheet.textContent =
      "html.veiled #app{filter:blur(16px) saturate(.9);transition:filter .16s ease}";
    (document.head ?? document.documentElement).append(sheet);
  };

  /** Blur only while the window is in the background, unless pinned. */
  const veiledNow = () =>
    memory.enabled && (memory.pinned || !document.hasFocus());

  const paintVeil = () => {
    mountSheet();
    document.documentElement.classList.toggle("veiled", veiledNow());
  };

  const readBadge = () => {
    const label = document.title ?? "";
    const bracketed = label.match(/\((\d+)\)/);
    if (bracketed) return Number(bracketed[1]) || 0;
    return /^[•*]/.test(label.trim()) ? 1 : 0;
  };

  const trackBadge = () => {
    if (ticker) return;
    ticker = setInterval(() => {
      const seen = readBadge();
      if (seen === memory.badge) return;
      memory.badge = seen;
      call("report_unread", { total: seen });
    }, 1000);
  };

  globalThis.__whara = {
    setBlur(next) {
      memory.enabled = Boolean(next);
      if (!memory.enabled) memory.pinned = false;
      paintVeil();
      return memory.enabled;
    },
    async toggleBlur() {
      memory.enabled = !memory.enabled;
      memory.pinned = false;
      paintVeil();
      // Keep the stored preference in sync with the shortcut / menu.
      await call("store_config", { patch: { privacyBlur: memory.enabled } });
      return memory.enabled;
    },
    blurred() {
      return veiledNow();
    },
  };

  addEventListener("focus", paintVeil, true);
  addEventListener("blur", paintVeil, true);
  document.addEventListener("visibilitychange", paintVeil);

  addEventListener(
    "keydown",
    (press) => {
      if (!press.metaKey || !press.shiftKey) return;
      if (String(press.key).toLowerCase() !== "b") return;
      press.preventDefault();
      globalThis.__whara.toggleBlur();
    },
    true,
  );

  const start = async () => {
    mountSheet();
    trackBadge();
    const stored = await call("read_config");
    if (!stored) return;
    if (typeof stored.privacyBlur === "boolean") {
      memory.enabled = stored.privacyBlur;
      paintVeil();
    }
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start, { once: true });
  } else {
    start();
  }
})();