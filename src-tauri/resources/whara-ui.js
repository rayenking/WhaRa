// WhaRa UI enhancements for WhatsApp Web.
// Keep native WhatsApp nodes in place. Only add controls owned by WhaRa.
// The WhatsApp DOM is not an API; all enhancements must fail open.
(() => {
  if (!/(^|\.)whatsapp\.com$/.test(location.hostname)) return;
  if (globalThis.__wharaUi) return;

  const SMALL = 800; // Switch before WhatsApp's native two-pane minimum (~748px).
  const flags = { compact: true, lists: true, calls: true };
  const state = {
    showList: false,
    lastMain: null,
    lastSide: null,
    lastChatKey: null,
    menuOpen: false,
    listOriginal: null,
    videoOriginal: null,
    voiceOriginal: null,
    voiceHost: null,
    callHost: null,
    listMenuRequestedAt: 0,
  };
  let style = null;
  let backButton = null;
  let listButton = null;
  let callButton = null;
  let menu = null;
  let mediaHost = null;
  let mediaHeader = null;
  let mediaTrigger = null;
  let mediaMenu = null;
  let mediaSearchCancel = null;
  let interval = null;
  let stopped = false;

  const css = `
    html.whara-compact #app .two {
      min-width: 0 !important;
      width: 100% !important;
      max-width: 100% !important;
    }
    html.whara-compact #app .two [data-whara-pane="side"],
    html.whara-compact-chat #app .two [data-whara-pane="side"] {
      min-width: 0 !important;
    }
    html.whara-compact-chat #app [data-whara-pane="side"] {
      display: none !important;
    }
    html.whara-compact-list #app [data-whara-pane="main"] {
      display: none !important;
    }
    html.whara-compact-chat #app [data-whara-pane="main"],
    html.whara-compact-list #app [data-whara-pane="side"] {
      flex: 1 1 0 !important;
      width: auto !important;
      min-width: 0 !important;
      max-width: 100% !important;
    }
    html.whara-compact-chat #app #main,
    html.whara-compact-list #app #side {
      flex: 1 1 auto !important;
      width: 100% !important;
      min-width: 0 !important;
      max-width: 100% !important;
    }
    /* Non-chat rail sections (Calls, Status, Channels, Communities and
       Settings) use WhatsApp's fullscreen drawer. In compact mode its
       left column remains capped at 45%, leaving the list and Settings
       cramped beside an empty middle column. Expand only a populated left
       column; the empty Chats drawer and native desktop layout stay intact. */
    html.whara-compact #app [data-testid="drawer-left"]:has([data-testid]) {
      flex: 1 1 100% !important;
      width: 100% !important;
      min-width: 0 !important;
      max-width: 100% !important;
      border-inline: 0 !important;
      box-shadow: none !important;
    }
    html.whara-compact #app [data-testid="drawer-left"]:has([data-testid]) > * {
      min-width: 0 !important;
      max-width: 100% !important;
    }
    /* Empty-state content in the second column is desktop-only. Hide it
       rather than letting it steal room from Calls/Settings/etc. */
    html.whara-compact #app [data-testid="drawer-left"]:has([data-testid])
      ~ [data-testid="drawer-middle"]:has([data-testid="empty-state-drawer"]) {
      display: none !important;
    }
    /* If a real detail view replaces the empty state, keep WhatsApp's
       actual content and handlers, but present it as a full-width overlay
       instead of squeezing both columns into 406px. */
    html.whara-compact #app [data-testid="drawer-left"]:has([data-testid])
      ~ [data-testid="drawer-middle"]:not(:has([data-testid="empty-state-drawer"])) {
      position: absolute !important;
      inset: 0 !important;
      display: block !important;
      flex: 0 0 100% !important;
      width: 100% !important;
      min-width: 0 !important;
      max-width: 100% !important;
      height: 100% !important;
      z-index: 350 !important;
    }
    /* A populated third pane (e.g. native account/contact details opened
       from a sidebar section) is another desktop split. Keep its own Close
       and Back actions but give the details the entire compact content area. */
    html.whara-compact #app [data-testid="drawer-left"]:has([data-testid])
      ~ [data-testid="drawer-right"]:has([data-testid]) {
      position: fixed !important;
      inset-block: 0 !important;
      left: var(--whara-content-left, 64px) !important;
      right: 0 !important;
      flex: 0 0 auto !important;
      width: calc(100vw - var(--whara-content-left, 64px)) !important;
      min-width: 0 !important;
      max-width: calc(100vw - var(--whara-content-left, 64px)) !important;
      height: 100vh !important;
      overflow: hidden !important;
      transform: none !important;
      z-index: 1100 !important;
    }
    html.whara-compact #app [data-testid="drawer-left"]:has([data-testid])
      ~ [data-testid="drawer-right"]:has([data-testid]) > * {
      width: 100% !important;
      min-width: 0 !important;
      max-width: 100% !important;
    }
    /* The floating Back-to-chats button belongs to a selected conversation,
       not the rail's Settings/Calls/etc. Avoid placing it over a drawer. */
    html.whara-compact:has(#app [data-testid="drawer-left"] [data-testid]) #whara-back {
      display: none !important;
    }
    /* Media rail button opens a separate native modal with a hard 748px
       dialog / 700px content minimum. Keep that portal inside the compact
       viewport instead of clipping the Media, Docs and Links tabs. */
    html.whara-compact #app [role="dialog"]:has([data-testid="media-hub-modal"]) {
      width: 100% !important;
      min-width: 0 !important;
      max-width: 100vw !important;
    }
    html.whara-compact #app [data-testid="media-hub-modal"] {
      flex: 0 1 auto !important;
      width: calc(100vw - 24px) !important;
      min-width: 0 !important;
      max-width: calc(100vw - 24px) !important;
      max-height: calc(100vh - 28px) !important;
    }
    html.whara-compact #app [data-testid="media-hub-modal"] > * {
      min-width: 0 !important;
      max-width: 100% !important;
    }
    /* Media / Docs / Links are a dropdown only in compact view. Keep
       WhatsApp's real tablist in the DOM, so each option still calls its
       native React handler. Avoid the 210px tab strip colliding with
       Search, Sort, Select and Close on a 470px window. */
    html.whara-compact #app [data-whara-media-header] > :first-child {
      flex: 1 1 0 !important;
      min-width: 0 !important;
    }
    html.whara-compact #app [data-whara-media-header] > :last-child {
      flex: 0 0 184px !important;
      width: 184px !important;
      min-width: 184px !important;
      max-width: 184px !important;
    }
    html.whara-compact #app [data-whara-media-tabs-host] {
      display: flex !important;
      align-self: center !important;
      align-items: center !important;
      justify-content: center !important;
      flex: 0 0 94px !important;
      width: 94px !important;
      min-width: 94px !important;
      max-width: 94px !important;
      overflow: visible !important;
    }
    html.whara-compact #app [data-whara-media-tabs-host] > [role="tablist"] {
      display: none !important;
    }
    #whara-media-trigger {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      gap: 5px;
      height: 38px;
      box-sizing: border-box;
      max-width: 94px;
      padding: 0 7px;
      border: 0;
      border-radius: 8px;
      background: transparent;
      color: var(--primary-strong, #f0f2f5);
      font: 500 14px system-ui, sans-serif;
      cursor: pointer;
      white-space: nowrap;
    }
    #whara-media-trigger:hover,
    #whara-media-trigger:focus-visible {
      background: var(--background-default-hover, rgba(120,130,140,.18));
    }
    #whara-media-trigger .whara-media-caret { font-size: 11px; }
    #whara-media-menu {
      position: fixed;
      z-index: 10005;
      box-sizing: border-box;
      width: 130px;
      padding: 5px;
      border: 1px solid rgba(140,140,140,.18);
      border-radius: 10px;
      color: var(--primary-strong, #f0f2f5);
      background: var(--panel-header-background, #202c33);
      box-shadow: 0 6px 24px rgba(0,0,0,.24);
    }
    #whara-media-menu button {
      display: flex;
      align-items: center;
      width: 100%;
      min-height: 36px;
      padding: 7px 10px;
      border: 0;
      border-radius: 6px;
      color: inherit;
      background: transparent;
      font: 14px system-ui, sans-serif;
      text-align: start;
      cursor: pointer;
    }
    #whara-media-menu button[aria-checked="true"],
    #whara-media-menu button:hover,
    #whara-media-menu button:focus-visible {
      background: var(--background-default-hover, rgba(120,130,140,.18));
    }
    /* While WhatsApp's native Media search is open, the title, category
       dropdown, Sort/Select and modal Close should yield their space.
       The native search (including its own clear/close control) then spans
       the full compact header. Never move/recreate the React search input. */
    html.whara-compact #app [data-whara-media-header][data-whara-media-search] {
      position: relative !important;
      min-height: 65px !important;
      width: 100% !important;
    }
    html.whara-compact #app [data-whara-media-header][data-whara-media-search] > :first-child,
    html.whara-compact #app [data-whara-media-header][data-whara-media-search]
      > [data-whara-media-tabs-host] {
      display: none !important;
    }
    html.whara-compact #app [data-whara-media-header][data-whara-media-search] > :last-child {
      position: absolute !important;
      inset: 8px 16px !important;
      display: flex !important;
      flex: 1 1 100% !important;
      width: auto !important;
      max-width: none !important;
      min-width: 0 !important;
      height: 48px !important;
      align-items: center !important;
    }
    html.whara-compact #app [data-whara-media-header][data-whara-media-search]
      > :last-child > :not(:first-child) {
      display: none !important;
    }
    html.whara-compact #app [data-whara-media-header][data-whara-media-search]
      > :last-child > :first-child,
    html.whara-compact #app [data-whara-media-header][data-whara-media-search]
      > :last-child > :first-child :has(input[role="textbox"]) {
      position: relative !important;
      flex: 1 1 0 !important;
      min-width: 0 !important;
      max-width: 100% !important;
      width: 100% !important;
    }
    html.whara-compact #app [data-whara-media-header][data-whara-media-search]
      input[role="textbox"] {
      box-sizing: border-box !important;
      flex: 1 1 auto !important;
      width: 100% !important;
      min-width: 0 !important;
      padding-right: 36px !important;
    }
    #whara-media-search-close {
      position: absolute;
      z-index: 1;
      top: 4px;
      right: 6px;
      display: inline-flex;
      width: 32px;
      height: 32px;
      align-items: center;
      justify-content: center;
      border: 0;
      border-radius: 50%;
      background: transparent;
      color: var(--primary-strong, #f0f2f5);
      cursor: pointer;
      font: 24px system-ui, sans-serif;
    }
    #whara-media-search-close:hover,
    #whara-media-search-close:focus-visible {
      background: var(--background-default-hover, rgba(120,130,140,.18));
    }
    #whara-media-search-close svg { width: 20px; height: 20px; }
    /* WhatsApp keeps a fullscreen drawer with its own desktop split layout
       even in compact chat mode. Its left/middle panes carry 1px dividers. */
    html.whara-compact-chat #app [data-testid="drawer-fullscreen"] {
      width: calc(100vw - var(--whara-content-left, 64px)) !important;
      min-width: 0 !important;
      max-width: calc(100vw - var(--whara-content-left, 64px)) !important;
    }
    html.whara-compact-chat #app [data-testid="drawer-left"],
    html.whara-compact-chat #app [data-testid="drawer-middle"] {
      border-inline: 0 !important;
      box-shadow: none !important;
    }
    /* Native contact/group info lives in drawer-right, but its parent retains
       the desktop 748px coordinate system. Overlay it over the chat content
       instead of allowing a second desktop split panel to overflow. */
    html.whara-compact-info #app [data-testid="drawer-right"] {
      position: fixed !important;
      inset-block: 0 !important;
      left: var(--whara-content-left, 64px) !important;
      right: 0 !important;
      width: calc(100vw - var(--whara-content-left, 64px)) !important;
      min-width: 0 !important;
      max-width: calc(100vw - var(--whara-content-left, 64px)) !important;
      height: 100vh !important;
      transform: none !important;
      overflow: hidden !important;
      z-index: 1100 !important;
    }
    html.whara-compact-info #app [data-testid="drawer-right"] > * {
      width: 100% !important;
      min-width: 0 !important;
      max-width: 100% !important;
    }
    /* Contact info and media/gallery headers should have breathing room
       between WhatsApp's native Close/Back button and the title. */
    html.whara-compact-info #app [data-testid="contact-info-header"] {
      margin-inline-start: 10px !important;
      min-width: 0 !important;
    }
    /* WhatsApp photo viewer uses z-index: 500, while our single-pane info
       overlay sits at 1100. Lift the *native* viewer above it temporarily. */
    html.whara-compact #app [data-whara-photo-viewer="true"] {
      z-index: 1600 !important;
    }
    html.whara-compact-chat #app #main header {
      padding-inline-start: 49px !important;
      min-width: 0 !important;
    }
    /* Retain the original WhatsApp header layout and contact styling.
       Only bring the combined call control closer to Search. */
    html.whara-compact-chat #main header [data-whara-call-host] {
      margin-inline-end: -10px !important;
    }
    html.whara-compact-chat #main header button[aria-label="Search"] {
      margin-inline-start: -12px !important;
    }
    /* WhatsApp applies >50 px of padding on each side of message wrappers,
       even in compact mode. Preserve row alignment but reduce that padding. */
    html.whara-compact-chat #main [data-testid^="conv-msg-"] :has(> [data-testid="msg-container"]) {
      padding-inline: 10px !important;
    }
    .whara-action {
      appearance: none;
      position: fixed;
      z-index: 950;
      box-sizing: border-box;
      display: flex;
      align-items: center;
      justify-content: center;
      width: 40px;
      height: 40px;
      border: 0;
      border-radius: 12px;
      background: transparent;
      color: var(--icon-high-emphasis, #aebac1);
      cursor: pointer;
      font: 500 19px system-ui, sans-serif;
    }
    .whara-action:hover,
    .whara-action:focus-visible {
      background: var(--background-default-hover, rgba(120,130,140,.18));
      outline-offset: 2px;
    }
    #whara-back {
      z-index: 960;
      font-size: 27px;
      line-height: 1;
    }
    #whara-call {
      position: relative;
      width: 40px;
      height: 40px;
      border-radius: 8px;
      gap: 2px;
      font-size: 20px;
    }
    #whara-call svg { width: 21px; height: 21px; flex: none; }
    #whara-call .whara-caret { font-size: 10px; line-height: 1; }
    #main header [data-whara-call-host] {
      flex: 0 0 42px !important;
      width: 42px !important;
      min-width: 42px !important;
    }
    .whara-native-action-hidden,
    #main header .whara-native-call-host-hidden { display: none !important; }
    #whara-call-menu {
      position: fixed;
      z-index: 10000;
      box-sizing: border-box;
      width: 144px;
      padding: 5px;
      border: 1px solid rgba(140,140,140,.18);
      border-radius: 10px;
      background: var(--panel-header-background, #202c33);
      color: var(--primary-strong, #f0f2f5);
      box-shadow: 0 6px 24px rgba(0,0,0,.24);
    }
    #whara-call-menu button {
      display: flex;
      align-items: center;
      gap: 9px;
      box-sizing: border-box;
      width: 100%;
      padding: 9px 10px;
      border: 0;
      border-radius: 6px;
      text-align: start;
      font: 14px system-ui, sans-serif;
      color: inherit;
      background: transparent;
      cursor: pointer;
    }
    #whara-add-to-list svg {
      display: block !important;
      margin: 0 !important;
      padding: 0 !important;
      width: 24px !important;
      height: 24px !important;
      flex: none;
      color: inherit !important;
      fill: currentColor !important;
    }
    #whara-add-to-list svg path { fill: currentColor !important; }
    #whara-call svg {
      color: inherit !important;
      fill: currentColor !important;
    }
    #whara-call-menu button svg { width: 20px; height: 20px; flex: none; }
    #whara-call-menu button:hover,
    #whara-call-menu button:focus-visible {
      background: var(--background-default-hover, rgba(130,140,150,.18));
    }
  `;

  function mountStyle() {
    if (style?.isConnected || !document.documentElement) return;
    style = document.createElement("style");
    style.id = "whara-ui-styles";
    style.textContent = css;
    (document.head || document.documentElement).append(style);
  }

  function action(id, label) {
    const button = document.createElement("button");
    button.type = "button";
    button.id = id;
    button.className = "whara-action";
    button.setAttribute("aria-label", label);
    button.title = label;
    return button;
  }

  function place(button, rect, offsetX = 0, offsetY = 0) {
    if (!button || !rect) return;
    button.style.left = Math.round(rect.left + offsetX) + "px";
    button.style.top = Math.round(rect.top + offsetY) + "px";
  }

  function clearCompact() {
    const root = document.documentElement;
    root.classList.remove("whara-compact", "whara-compact-chat", "whara-compact-list", "whara-compact-info");
    root.style.removeProperty("--whara-content-left");
    document.querySelector('[data-whara-photo-viewer]')?.removeAttribute('data-whara-photo-viewer');
    backButton?.remove();
  }

  function compactLayout() {
    const side = document.querySelector("#side");
    const main = document.querySelector("#main");
    if (side?.parentElement && side !== state.lastSide) {
      state.lastSide?.parentElement?.removeAttribute("data-whara-pane");
      side.parentElement.setAttribute("data-whara-pane", "side");
      state.lastSide = side;
    }
    // WhatsApp keeps a right-hand placeholder pane even with no active #main.
    // Mark the sibling as the main pane so compact list mode can hide it too.
    const otherPane = side?.parentElement?.parentElement?.classList.contains("two")
      ? side.parentElement.nextElementSibling : null;
    if (otherPane && (!main || otherPane === main.parentElement)) {
      otherPane.setAttribute("data-whara-pane", "main");
    }
    if (main?.parentElement && main !== state.lastMain) {
      state.lastMain?.parentElement?.removeAttribute("data-whara-pane");
      main.parentElement.setAttribute("data-whara-pane", "main");
      state.lastMain = main;
      state.showList = false; // Newly mounted chat takes focus.
    }
    if (!flags.compact || innerWidth >= SMALL || !side) {
      clearCompact();
      state.showList = false;
      return;
    }
    const root = document.documentElement;
    root.classList.add("whara-compact");
    const showChat = Boolean(main) && !state.showList;
    const activePane = showChat ? main : side;
    root.style.setProperty("--whara-content-left",
      Math.max(0, Math.round(activePane.getBoundingClientRect().left)) + "px");
    root.classList.toggle("whara-compact-chat", showChat);
    root.classList.toggle("whara-compact-list", !showChat);
    if (!showChat) {
      backButton?.remove();
      return;
    }
    const header = main.querySelector("header");
    if (!header) return;
    if (!backButton) {
      backButton = action("whara-back", "Back to chats");
      backButton.textContent = "‹";
      backButton.addEventListener("click", () => {
        state.showList = true;
        closeMenu();
        sync();
      });
    }
    if (!backButton.isConnected) document.body.appendChild(backButton);
    const rect = header.getBoundingClientRect();
    place(backButton, rect, 8, Math.max(0, (rect.height - 40) / 2));
  }

  function profileDrawer() {
    const root = document.documentElement;
    const right = document.querySelector('#app [data-testid="drawer-right"]');
    // Contact info has a Close button, but Media / Links / Docs replaces
    // that content with a nested gallery drawer whose control is Back.
    // Detect both without depending on measured width (our CSS changes it).
    const infoPanel = !!right?.querySelector('button[aria-label="Close"]');
    const galleryPanel = !!right?.querySelector('[data-testid="media-gallery-drawer"]');
    const open = root.classList.contains("whara-compact-chat") &&
      (infoPanel || galleryPanel);
    root.classList.toggle("whara-compact-info", open);
    // Otherwise our fixed Back-to-chats button overlaps native Close/Back.
    if (open) {
      backButton?.remove();
      closeMenu();
    }

    // The native photo viewer is a fixed overlay (z-index:500) in WhatsApp's
    // portal. It must be above the compact contact drawer (z-index:1100).
    // Find its existing Close button and climb to the actual portal root.
    // No React-owned elements are moved or replaced.
    const allCloses = document.querySelectorAll('#app button[aria-label="Close"]');
    for (const button of allCloses) {
      if (button.closest('[data-testid="drawer-right"]')) continue;
      let parent = button.parentElement;
      for (let depth = 0; parent && depth < 9; depth++, parent = parent.parentElement) {
        const s = getComputedStyle(parent);
        const rect = parent.getBoundingClientRect();
        if (s.position !== "fixed" || s.zIndex !== "500" ||
            rect.width < innerWidth - 12 || rect.height < innerHeight - 12) continue;
        if (parent.querySelector('img')) {
          parent.setAttribute('data-whara-photo-viewer', 'true');
        }
        break;
      }
    }
  }

  function listAction() {
    // The pre-video button is actually WhatsApp's "Add to list" action,
    // not its Media section. Media already has its own native rail button.
    const original = document.querySelector('#main header button[data-testid="label-chat-header-button"][aria-label="Add to list"]');
    const nativeMedia = document.querySelector('button[aria-label="Media"]');
    if (!flags.lists || !original || !nativeMedia ||
        document.documentElement.classList.contains("whara-compact-list")) {
      state.listOriginal?.classList.remove("whara-native-action-hidden");
      state.listOriginal = null;
      listButton?.remove();
      return;
    }
    if (!listButton) {
      listButton = action("whara-add-to-list", "Add to list");
      listButton.addEventListener("click", () => {
        const native = state.listOriginal;
        if (native?.isConnected && !native.disabled) {
          // The native dropdown is portaled into the app wrapper but its
          // positioning is still anchored to the now-hidden header button.
          state.listMenuRequestedAt = Date.now();
          native.click();
          requestAnimationFrame(positionListMenu);
        }
      });
    }
    // The reference points to the native action; no React-owned node moves.
    if (state.listOriginal !== original) {
      state.listOriginal?.classList.remove("whara-native-action-hidden");
      state.listOriginal = original;
      const icon = original.querySelector("svg")?.cloneNode(true);
      listButton.replaceChildren(icon || document.createTextNode("▤"));
      if (icon) {
        // The original header icon has obfuscated utility classes that add
        // an offset inside the 40px sidebar hit target. Strip only the clone.
        icon.removeAttribute("class");
        icon.setAttribute("width", "24");
        icon.setAttribute("height", "24");
        icon.setAttribute("aria-hidden", "true");
      }
    }
    const r = nativeMedia.getBoundingClientRect();
    const railUpper = document.querySelector('button[aria-label="Advertise"]')?.getBoundingClientRect();
    // Never cover existing rail controls in a short window.
    if (railUpper && r.top - railUpper.bottom < 50) {
      listButton.remove();
      original.classList.remove("whara-native-action-hidden");
      return;
    }
    if (!listButton.isConnected) document.body.appendChild(listButton);
    listButton.disabled = original.disabled;
    // Match the actual native rail icon color in the active light/dark theme.
    listButton.style.color = getComputedStyle(nativeMedia).color;
    place(listButton, r, 0, -44);
    original.classList.add("whara-native-action-hidden");
  }

  function positionListMenu() {
    if (!state.listMenuRequestedAt) return;
    const item = document.querySelector('[data-testid="labels-dropdown-new-list"]');
    const popup = item?.closest('[role="menu"]')?.parentElement;
    if (!popup || !listButton?.isConnected) {
      if (Date.now() - state.listMenuRequestedAt > 1800) state.listMenuRequestedAt = 0;
      return;
    }
    // The real WhatsApp popup is an absolutely positioned portal under the
    // app wrapper, with native "right" coordinates from the former header.
    // Keep React's menu, options and click handlers, only override its anchor.
    const r = listButton.getBoundingClientRect();
    const w = popup.getBoundingClientRect().width;
    const h = popup.getBoundingClientRect().height;
    const wrapper = popup.offsetParent?.getBoundingClientRect();
    if (!w || !h || !wrapper) return;
    const left = Math.max(8, Math.min(innerWidth - w - 8, r.right + 10));
    const top = Math.max(8, Math.min(innerHeight - h - 8, r.top + r.height / 2 - h / 2));
    const x = Math.round(left - wrapper.left) + "px";
    const y = Math.round(top - wrapper.top) + "px";
    if (popup.style.getPropertyValue("left") !== x ||
        popup.style.getPropertyPriority("left") !== "important") {
      popup.style.setProperty("left", x, "important");
      popup.style.setProperty("right", "auto", "important");
    }
    if (popup.style.getPropertyValue("top") !== y ||
        popup.style.getPropertyPriority("top") !== "important") {
      popup.style.setProperty("top", y, "important");
    }
    if (popup.style.getPropertyValue("transform-origin") !== "left center") {
      popup.style.setProperty("transform-origin", "left center", "important");
    }
  }

  function syncActionColors() {
    // --icon-high-emphasis maps to green in this WhatsApp build; use the
    // actual Search icon color instead to keep the header monochrome.
    const search = document.querySelector('#main header button[aria-label="Search"]');
    if (!search) return;
    const nativeColor = getComputedStyle(search).color;
    if (backButton) backButton.style.color = nativeColor;
    if (callButton) callButton.style.color = nativeColor;
  }

  function closeMenu() {
    state.menuOpen = false;
    menu?.remove();
    if (callButton) callButton.setAttribute("aria-expanded", "false");
  }

  function callAction() {
    const video = document.querySelector('#main header button[aria-label="Video call"]');
    const voice = document.querySelector('#main header button[aria-label="Voice call"]');
    if (!flags.calls || !video || !voice || !video.parentElement) {
      state.videoOriginal?.classList.remove("whara-native-action-hidden");
      state.voiceOriginal?.classList.remove("whara-native-action-hidden");
      state.voiceHost?.classList.remove("whara-native-call-host-hidden");
      state.callHost?.removeAttribute("data-whara-call-host");
      state.videoOriginal = null;
      state.voiceOriginal = null;
      state.voiceHost = null;
      state.callHost = null;
      callButton?.remove();
      closeMenu();
      return;
    }
    if (!callButton) {
      callButton = action("whara-call", "Voice and video calling");
      // A native WhatsApp SVG is inserted below once both call actions exist.
      callButton.textContent = "☎";
      callButton.setAttribute("aria-haspopup", "menu");
      callButton.setAttribute("aria-expanded", "false");
      callButton.addEventListener("click", () => {
        if (state.menuOpen) {
          closeMenu();
          return;
        }
        menu = document.createElement("div");
        menu.id = "whara-call-menu";
        menu.setAttribute("role", "menu");
        for (const [label, key] of [["Voice", "voice"], ["Video", "video"]]) {
          const option = document.createElement("button");
          option.type = "button";
          option.setAttribute("role", "menuitem");
          const original = key === "voice" ? state.voiceOriginal : state.videoOriginal;
          const icon = original?.querySelector("svg")?.cloneNode(true);
          if (icon) {
            icon.setAttribute("aria-hidden", "true");
            option.append(icon);
          }
          option.append(document.createTextNode(label));
          option.disabled = !original?.isConnected || original.disabled;
          option.addEventListener("click", () => {
            const original = key === "voice" ? state.voiceOriginal : state.videoOriginal;
            closeMenu();
            if (original?.isConnected && !original.disabled) original.click();
          });
          menu.append(option);
        }
        // Use the actual WhatsApp chat-header background (not the bluer
        // dropdown token), so this matches the active WhatsApp theme.
        const header = document.querySelector("#main header");
        for (let node = header; node; node = node.parentElement) {
          const background = getComputedStyle(node).backgroundColor;
          if (background && background !== "transparent" &&
              background !== "rgba(0, 0, 0, 0)") {
            menu.style.backgroundColor = background;
            break;
          }
        }
        document.body.append(menu);
        const r = callButton.getBoundingClientRect();
        menu.style.top = Math.min(innerHeight - menu.offsetHeight - 8, r.bottom + 4) + "px";
        menu.style.left = Math.max(8, Math.min(innerWidth - menu.offsetWidth - 8, r.left)) + "px";
        state.menuOpen = true;
        callButton.setAttribute("aria-expanded", "true");
        menu.querySelector("button")?.focus();
      });
    }
    if (state.videoOriginal !== video || state.voiceOriginal !== voice) {
      state.videoOriginal?.classList.remove("whara-native-action-hidden");
      state.voiceOriginal?.classList.remove("whara-native-action-hidden");
      closeMenu();
      state.videoOriginal = video;
      state.voiceOriginal = voice;
      const nativeIcon = voice.querySelector("svg")?.cloneNode(true);
      const caret = document.createElement("span");
      caret.className = "whara-caret";
      caret.setAttribute("aria-hidden", "true");
      caret.textContent = "▾";
      if (nativeIcon) {
        nativeIcon.setAttribute("aria-hidden", "true");
        nativeIcon.removeAttribute("class");
      }
      callButton.replaceChildren(nativeIcon || document.createTextNode("☎"), caret);
    }
    const host = video.parentElement;
    const voiceHost = voice.parentElement;
    if (state.callHost !== host) {
      state.callHost?.removeAttribute("data-whara-call-host");
      callButton.remove();
      state.callHost = host;
    }
    if (state.voiceHost !== voiceHost) {
      state.voiceHost?.classList.remove("whara-native-call-host-hidden");
      state.voiceHost = voiceHost;
    }
    host.setAttribute("data-whara-call-host", "");
    if (callButton.parentElement !== host) host.append(callButton);
    if (voiceHost !== host) voiceHost.classList.add("whara-native-call-host-hidden");
    video.classList.add("whara-native-action-hidden");
    voice.classList.add("whara-native-action-hidden");
  }

  const MEDIA_TABS = [
    ["Media", "tab-media"],
    ["Docs", "tab-docs"],
    ["Links", "tab-links"],
  ];

  function closeMediaMenu(focusTrigger = false) {
    mediaMenu?.remove();
    mediaMenu = null;
    mediaTrigger?.setAttribute("aria-expanded", "false");
    if (focusTrigger) mediaTrigger?.focus();
  }

  function unmountMediaTabs() {
    closeMediaMenu();
    mediaSearchCancel?.remove();
    mediaHost?.removeAttribute("data-whara-media-tabs-host");
    mediaHeader?.removeAttribute("data-whara-media-search");
    mediaHeader?.removeAttribute("data-whara-media-header");
    mediaTrigger?.remove();
    mediaHost = null;
    mediaHeader = null;
  }

  function selectedMediaTab(modal) {
    const selected = MEDIA_TABS.find(([, id]) =>
      modal.querySelector('[data-testid="' + id + '"]')?.getAttribute("aria-selected") === "true");
    return selected || MEDIA_TABS[0];
  }

  function placeMediaMenu() {
    if (!mediaMenu?.isConnected || !mediaTrigger?.isConnected) return;
    const rect = mediaTrigger.getBoundingClientRect();
    mediaMenu.style.left = Math.max(8,
      Math.min(innerWidth - mediaMenu.offsetWidth - 8, rect.left)) + "px";
    mediaMenu.style.top = Math.max(8,
      Math.min(innerHeight - mediaMenu.offsetHeight - 8, rect.bottom + 5)) + "px";
  }

  function openMediaMenu() {
    if (!mediaTrigger?.isConnected) return;
    if (mediaMenu) {
      closeMediaMenu();
      return;
    }
    const modal = mediaTrigger.closest('[data-testid="media-hub-modal"]');
    if (!modal) return;
    closeMenu();
    const selected = selectedMediaTab(modal)[1];
    const popup = document.createElement("div");
    popup.id = "whara-media-menu";
    popup.setAttribute("role", "menu");
    popup.setAttribute("aria-label", "Media categories");
    for (const [label, testid] of MEDIA_TABS) {
      const original = modal.querySelector('[data-testid="' + testid + '"]');
      if (!original) continue;
      const option = document.createElement("button");
      option.type = "button";
      option.setAttribute("role", "menuitemradio");
      option.setAttribute("aria-checked", String(testid === selected));
      option.textContent = label;
      option.addEventListener("click", () => {
        closeMediaMenu();
        // Call WhatsApp's original tab handler; never replace its React content.
        const live = modal.querySelector('[data-testid="' + testid + '"]');
        if (live?.isConnected) live.click();
        requestAnimationFrame(syncMediaTabs);
      });
      popup.append(option);
    }
    if (!popup.childElementCount) return;
    // The Media header itself is transparent; the modal owns its real
    // background color. Match it instead of using the bluer dropdown token.
    const background = getComputedStyle(modal).backgroundColor;
    if (background && background !== "transparent" &&
        background !== "rgba(0, 0, 0, 0)") {
      popup.style.backgroundColor = background;
    }
    document.body.append(popup);
    mediaMenu = popup;
    mediaTrigger.setAttribute("aria-expanded", "true");
    placeMediaMenu();
    popup.querySelector('[aria-checked="true"]')?.focus();
  }

  function syncMediaTabs() {
    const modal = document.querySelector('#app [data-testid="media-hub-modal"]');
    const nativeTab = modal?.querySelector('[data-testid="tab-media"]');
    const tabList = nativeTab?.closest('[role="tablist"]');
    const host = tabList?.parentElement;
    const header = host?.parentElement;
    if (!document.documentElement.classList.contains("whara-compact") ||
        !modal || !host || !header ||
        !MEDIA_TABS.every(([, id]) => modal.querySelector('[data-testid="' + id + '"]'))) {
      if (mediaHost || mediaTrigger?.isConnected || mediaMenu) unmountMediaTabs();
      return;
    }

    if (!mediaTrigger) {
      mediaTrigger = document.createElement("button");
      mediaTrigger.type = "button";
      mediaTrigger.id = "whara-media-trigger";
      mediaTrigger.title = "Choose Media, Docs or Links";
      mediaTrigger.setAttribute("aria-label", "Choose media category");
      mediaTrigger.setAttribute("aria-haspopup", "menu");
      mediaTrigger.setAttribute("aria-expanded", "false");
      const label = document.createElement("span");
      label.className = "whara-media-label";
      const caret = document.createElement("span");
      caret.className = "whara-media-caret";
      caret.setAttribute("aria-hidden", "true");
      caret.textContent = "▾";
      mediaTrigger.replaceChildren(label, caret);
      mediaTrigger.addEventListener("click", openMediaMenu);
      mediaTrigger.addEventListener("keydown", (event) => {
        if (event.key === "ArrowDown" || event.key === "Enter" && !mediaMenu) {
          event.preventDefault();
          if (!mediaMenu) openMediaMenu();
        }
      });
    }
    if (mediaHost !== host || mediaHeader !== header) {
      unmountMediaTabs();
      mediaHost = host;
      mediaHeader = header;
    }
    if (!mediaTrigger.isConnected || mediaTrigger.parentElement !== host) {
      host.append(mediaTrigger);
    }
    host.setAttribute("data-whara-media-tabs-host", "");
    header.setAttribute("data-whara-media-header", "");
    // WhatsApp mounts this textbox only while the native search is active.
    // Drive the expansion from actual DOM state, not focus or user typing.
    const searchInput = header.querySelector('input[role="textbox"]');
    const searching = !!searchInput;
    header.toggleAttribute("data-whara-media-search", searching);
    if (searching && mediaMenu) closeMediaMenu();
    if (searching) {
      const nativeSearch = header.lastElementChild?.firstElementChild;
      if (!mediaSearchCancel) {
        mediaSearchCancel = document.createElement("button");
        mediaSearchCancel.type = "button";
        mediaSearchCancel.id = "whara-media-search-close";
        mediaSearchCancel.setAttribute("aria-label", "Close media search");
        const closeIcon = modal.querySelector('button[aria-label="Close"] svg')?.cloneNode(true);
        if (closeIcon) {
          closeIcon.setAttribute("aria-hidden", "true");
          mediaSearchCancel.append(closeIcon);
        } else {
          mediaSearchCancel.textContent = "×";
        }
        mediaSearchCancel.addEventListener("click", () => {
          // Let WhatsApp close its own native search. Escape is handled by
          // the original input (rather than altering React-managed state).
          const input = mediaHeader?.querySelector('input[role="textbox"]');
          if (!input) return;
          input.dispatchEvent(new KeyboardEvent("keydown", {
            key: "Escape", code: "Escape", bubbles: true, cancelable: true,
          }));
          requestAnimationFrame(syncMediaTabs);
        });
      }
      if (nativeSearch && mediaSearchCancel.parentElement !== nativeSearch) {
        nativeSearch.append(mediaSearchCancel);
      }
    } else {
      mediaSearchCancel?.remove();
    }
    const [label] = selectedMediaTab(modal);
    const text = mediaTrigger.querySelector(".whara-media-label");
    if (text && text.textContent !== label) text.textContent = label;
    placeMediaMenu();
  }

  function onPageClick(event) {
    // The compact back button hides #main but does not close WhatsApp's
    // conversation. A chat-row click restores conversation mode.
    if (event.target instanceof Element && event.target.closest('#pane-side [role="gridcell"]')) {
      state.showList = false;
      queueMicrotask(sync);
    }
    if (state.menuOpen && !event.target.closest?.("#whara-call, #whara-call-menu")) closeMenu();
    if (mediaMenu && !event.target.closest?.("#whara-media-trigger, #whara-media-menu")) {
      closeMediaMenu();
    }
  }

  function onKey(event) {
    if (event.key === "Escape" && mediaMenu) {
      event.preventDefault();
      event.stopPropagation();
      closeMediaMenu(true);
      return;
    }
    if (event.key === "Escape") closeMenu();
    if ((event.key === "Enter" || event.key === " ") &&
        document.activeElement?.closest?.('#pane-side [role="gridcell"]')) {
      state.showList = false;
      queueMicrotask(sync);
    }
  }

  function sync() {
    if (stopped || !document.body) return;
    mountStyle();
    compactLayout();
    profileDrawer();
    listAction();
    callAction();
    syncMediaTabs();
    syncActionColors();
    positionListMenu();
  }

  function diagnostics() {
    const box = (selector) => {
      const e = document.querySelector(selector);
      if (!e) return null;
      const r = e.getBoundingClientRect();
      return { x: Math.round(r.x), width: Math.round(r.width),
        height: Math.round(r.height), clientHeight: e.clientHeight,
        scrollHeight: e.scrollHeight };
    };
    return {
      viewport: { width: innerWidth, height: innerHeight, scale: devicePixelRatio },
      documentScrollWidth: document.documentElement.scrollWidth,
      nativeZoom: getComputedStyle(document.documentElement).zoom,
      compactMode: document.documentElement.classList.contains("whara-compact-info")
        ? (document.querySelector('#app [data-testid="media-gallery-drawer"]')
          ? "media" : "info") : document.documentElement.classList.contains("whara-compact-chat")
          ? "chat" : document.documentElement.classList.contains("whara-compact-list")
            ? "list" : "native",
      flags: { ...flags },
      side: box("#side"), chat: box("#main"), chatList: box("#pane-side"),
      nativeMedia: !!document.querySelector('button[aria-label="Media"]'),
      addToList: !!state.listOriginal?.isConnected,
      unifiedCalls: !!callButton?.isConnected,
    };
  }

  function destroy() {
    stopped = true;
    if (interval) clearInterval(interval);
    removeEventListener("resize", sync);
    document.removeEventListener("click", onPageClick, true);
    document.removeEventListener("keydown", onKey, true);
    closeMenu();
    unmountMediaTabs();
    clearCompact();
    state.lastSide?.parentElement?.removeAttribute("data-whara-pane");
    state.lastMain?.parentElement?.removeAttribute("data-whara-pane");
    state.listOriginal?.classList.remove("whara-native-action-hidden");
    state.videoOriginal?.classList.remove("whara-native-action-hidden");
    state.voiceOriginal?.classList.remove("whara-native-action-hidden");
    state.voiceHost?.classList.remove("whara-native-call-host-hidden");
    state.callHost?.removeAttribute("data-whara-call-host");
    listButton?.remove();
    callButton?.remove();
    style?.remove();
    delete globalThis.__wharaUi;
  }

  const api = {
    diagnostics,
    setEnabled(feature, enabled) {
      if (!Object.hasOwn(flags, feature)) return false;
      flags[feature] = Boolean(enabled);
      sync();
      return true;
    },
    destroy,
  };
  globalThis.__wharaUi = api;
  if (globalThis.__whara) globalThis.__whara.ui = api;
  addEventListener("resize", sync);
  document.addEventListener("click", onPageClick, true);
  document.addEventListener("keydown", onKey, true);
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", sync, { once: true });
  } else {
    sync();
  }
  interval = setInterval(sync, 400);
})();
