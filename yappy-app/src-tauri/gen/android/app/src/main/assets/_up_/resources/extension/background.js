// Yappy bridge — service worker.
//
// Responsibilities:
//   1. Open one WebSocket connection to ws://127.0.0.1:47898 when needed.
//   2. Authenticate with the desktop app using the stored pairing token.
//   3. On toolbar-action click OR keyboard command, ask the active tab to
//      extract its clean content via defuddle and forward it to the app.

const BRIDGE_URL = "ws://127.0.0.1:47898";
let socket = null;
let pendingAfterOpen = [];
let lastStatus = "disconnected"; // "connecting" | "connected" | "not_paired" | "disconnected"
let browserName = detectBrowser();

function detectBrowser() {
  // chrome.runtime.getBrowserInfo only exists on Firefox; UA sniffing for chromium.
  const ua = navigator.userAgent || "";
  if (ua.includes("Vivaldi")) return "Vivaldi";
  if (ua.includes("Edg/")) return "Microsoft Edge";
  if (ua.includes("Brave") || (typeof navigator.brave !== "undefined")) return "Brave Browser";
  if (ua.includes("OPR/") || ua.includes("Opera")) return "Opera";
  if (ua.includes("Arc/")) return "Arc";
  if (ua.includes("Chrome/")) return "Google Chrome";
  return "Chromium";
}

async function getToken() {
  const { yappyToken } = await chrome.storage.local.get("yappyToken");
  if (yappyToken) return yappyToken;
  // First-run: generate a random token. Yappy's bridge auto-pairs on the first
  // unclaimed connection, so the very first hello locks the pairing in for the user.
  const fresh = crypto.randomUUID();
  await chrome.storage.local.set({ yappyToken: fresh });
  return fresh;
}

function setStatus(s) {
  lastStatus = s;
  chrome.runtime.sendMessage({ type: "status", status: s }).catch(() => {});
  const badge = {
    connected: { text: "✓", color: "#5dbb70" },
    connecting: { text: "…", color: "#8a8a8a" },
    not_paired: { text: "!", color: "#e84785" },
    disconnected: { text: "", color: "#cccccc" },
  }[s] || { text: "", color: "#cccccc" };
  chrome.action.setBadgeText({ text: badge.text }).catch(() => {});
  chrome.action.setBadgeBackgroundColor({ color: badge.color }).catch(() => {});
}

async function ensureSocket() {
  if (socket && socket.readyState === WebSocket.OPEN) return socket;
  if (socket && socket.readyState === WebSocket.CONNECTING) return socket;
  setStatus("connecting");
  socket = new WebSocket(BRIDGE_URL);

  socket.addEventListener("open", async () => {
    const token = await getToken();
    socket.send(JSON.stringify({ type: "hello", token, browser: browserName }));
  });
  socket.addEventListener("message", async (ev) => {
    let m;
    try { m = JSON.parse(ev.data); } catch { return; }
    if (m.type === "welcome" && m.paired) {
      setStatus("connected");
      const flushed = pendingAfterOpen;
      pendingAfterOpen = [];
      for (const p of flushed) socket.send(p);
    } else if (m.type === "not_paired") {
      setStatus("not_paired");
      try { socket.close(); } catch {}
    } else if (m.type === "ack") {
      // no-op
    } else if (m.type === "fetch_current_tab") {
      // Yappy pressed the global hotkey while the browser was focused — push the page.
      const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
      if (tab && tab.id) await captureAndSend(tab.id);
    }
  });
  socket.addEventListener("close", () => {
    if (lastStatus !== "not_paired") setStatus("disconnected");
    socket = null;
  });
  socket.addEventListener("error", () => {
    if (lastStatus !== "not_paired") setStatus("disconnected");
  });
  return socket;
}

async function sendPayload(payload) {
  await ensureSocket();
  const s = JSON.stringify(payload);
  if (socket.readyState === WebSocket.OPEN) socket.send(s);
  else pendingAfterOpen.push(s);
}

async function captureAndSend(tabId) {
  try {
    // Step 1: inject defuddle bundle into the page's MAIN world so it can read the DOM.
    await chrome.scripting.executeScript({
      target: { tabId },
      files: ["defuddle.js"],
      world: "MAIN",
    });
    // Step 2: run defuddle in the same MAIN world and read `Defuddle` from window.
    const results = await chrome.scripting.executeScript({
      target: { tabId },
      world: "MAIN",
      func: () => {
        try {
          const C = (typeof Defuddle !== "undefined") ? Defuddle : (window && window.Defuddle);
          if (!C) return { error: "defuddle missing in MAIN world" };
          const r = new C(document, { markdown: true, debug: false }).parse();
          return {
            title: r.title || document.title || "",
            content: r.markdownContent || r.content || "",
            url: location.href,
            wordCount: r.wordCount || 0,
          };
        } catch (e) {
          return { error: String(e && e.message ? e.message : e) };
        }
      },
    });
    const result = results && results[0] && results[0].result;
    if (!result || result.error) {
      console.warn("Yappy: defuddle failed —", result && result.error);
      return false;
    }
    if (!result.content || result.content.trim().length === 0) {
      console.warn("Yappy: defuddle returned empty content");
      return false;
    }
    console.log("Yappy: defuddle extracted", result.wordCount, "words from", result.title);
    await sendPayload({
      type: "page",
      url: result.url,
      title: result.title,
      markdown: result.content,
    });
    return true;
  } catch (e) {
    console.warn("Yappy: capture error", e);
    return false;
  }
}

chrome.action.onClicked.addListener(async (tab) => {
  if (!tab.id) return;
  await captureAndSend(tab.id);
});

chrome.commands.onCommand.addListener(async (command) => {
  if (command === "send-page") {
    const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
    if (tab && tab.id) await captureAndSend(tab.id);
  }
});

chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
  if (msg && msg.type === "ping-status") {
    sendResponse({ status: lastStatus, browser: browserName });
  } else if (msg && msg.type === "send-current") {
    chrome.tabs.query({ active: true, currentWindow: true }).then(([tab]) => {
      if (tab && tab.id) captureAndSend(tab.id).then((ok) => sendResponse({ ok }));
    });
    return true; // async
  } else if (msg && msg.type === "reconnect") {
    try { socket && socket.close(); } catch {}
    ensureSocket().then(() => sendResponse({ ok: true }));
    return true;
  }
});

// Auto-connect when token is saved.
chrome.storage.onChanged.addListener((changes) => {
  if (changes.yappyToken) {
    try { socket && socket.close(); } catch {}
    ensureSocket();
  }
});

// Try to connect at boot.
ensureSocket().catch(() => {});
