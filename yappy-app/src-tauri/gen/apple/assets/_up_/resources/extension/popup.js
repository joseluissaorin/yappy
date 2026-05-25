// Yappy extension popup.
//
// On open: query bridge status + ask the active tab for its defuddled content.
// Preview the cleaned title + first paragraphs. User clicks "read this page in
// yappy" to forward it, or "copy text" to grab a markdown copy.
const statusEl = document.getElementById("status");
const hintEl = document.getElementById("hint");
const sendEl = document.getElementById("send");
const copyEl = document.getElementById("copy");
const openOptions = document.getElementById("open-options");
const previewEl = document.getElementById("preview");
const previewTitleEl = document.getElementById("preview-title");
const previewStatsEl = document.getElementById("preview-stats");
const previewBodyEl = document.getElementById("preview-body");

let bridgeStatus = "connecting";
let extractedContent = null; // { title, content (markdown), url, wordCount }

function applyBridgeStatus(s) {
  bridgeStatus = s;
  statusEl.className = "status " + s;
  statusEl.textContent = ({
    connected: "connected",
    connecting: "connecting…",
    not_paired: "not paired",
    disconnected: "yappy off?",
  })[s] || s;
  refreshHint();
  refreshButtons();
}

function refreshHint() {
  // Hint changes depending on bridge status AND whether content is extracted.
  if (extractedContent && bridgeStatus === "connected") {
    hintEl.textContent = "ready — click below to read aloud.";
  } else if (extractedContent && bridgeStatus !== "connected") {
    hintEl.textContent = ({
      connecting: "looking for yappy on your mac…",
      not_paired: "open yappy → preferences → browser extension to pair.",
      disconnected: "is yappy running? open the app and reconnect.",
    })[bridgeStatus] || "";
  } else {
    hintEl.textContent = "extracting clean text…";
  }
}

function refreshButtons() {
  const haveContent = Boolean(extractedContent && extractedContent.content);
  sendEl.disabled = !haveContent || bridgeStatus !== "connected";
  copyEl.disabled = !haveContent;
}

async function extractActiveTab() {
  try {
    const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
    if (!tab || !tab.id) return;
    // Inject defuddle into MAIN world, then read it back.
    await chrome.scripting.executeScript({
      target: { tabId: tab.id },
      files: ["defuddle.js"],
      world: "MAIN",
    });
    const results = await chrome.scripting.executeScript({
      target: { tabId: tab.id },
      world: "MAIN",
      func: () => {
        try {
          const C = (typeof Defuddle !== "undefined") ? Defuddle : (window && window.Defuddle);
          if (!C) return { error: "defuddle missing" };
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
    if (!result || result.error || !result.content) {
      previewTitleEl.textContent = "couldn't extract this page";
      previewStatsEl.textContent = result?.error ?? "no text found";
      previewBodyEl.textContent = "";
      previewEl.hidden = false;
      return;
    }
    extractedContent = result;
    previewTitleEl.textContent = result.title || "(no title)";
    const charCount = result.content.length.toLocaleString();
    previewStatsEl.textContent = `${result.wordCount.toLocaleString()} words · ${charCount} chars`;
    // Show first ~3 paragraphs as a preview.
    const preview = result.content
      .split(/\n\n+/)
      .slice(0, 4)
      .join("\n\n")
      .slice(0, 1200);
    previewBodyEl.textContent = preview + (result.content.length > preview.length ? "\n\n…" : "");
    previewEl.hidden = false;
  } catch (e) {
    console.error("extractActiveTab failed", e);
    previewTitleEl.textContent = "extraction failed";
    previewStatsEl.textContent = String(e && e.message ? e.message : e);
    previewBodyEl.textContent = "";
    previewEl.hidden = false;
  } finally {
    refreshHint();
    refreshButtons();
  }
}

// Boot.
chrome.runtime.sendMessage({ type: "ping-status" }, (resp) => {
  if (resp) applyBridgeStatus(resp.status);
});
chrome.runtime.onMessage.addListener((msg) => {
  if (msg && msg.type === "status") applyBridgeStatus(msg.status);
});
extractActiveTab();

sendEl.addEventListener("click", () => {
  sendEl.disabled = true;
  // The background's send-current uses the same extraction it just did
  // (defuddle is fast + re-running is fine; we don't pass content from popup
  // to background to keep the message contract simple).
  chrome.runtime.sendMessage({ type: "send-current" }, () => {
    window.close();
  });
});

copyEl.addEventListener("click", async () => {
  if (!extractedContent?.content) return;
  try {
    await navigator.clipboard.writeText(
      (extractedContent.title ? extractedContent.title + "\n\n" : "") + extractedContent.content,
    );
    copyEl.textContent = "copied ✓";
    setTimeout(() => { copyEl.textContent = "copy text"; }, 1600);
  } catch (e) {
    console.warn("clipboard write failed", e);
  }
});

openOptions.addEventListener("click", (e) => {
  e.preventDefault();
  chrome.runtime.openOptionsPage();
});
