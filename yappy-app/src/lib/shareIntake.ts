// iOS Share-Sheet payload handler.
//
// When the user picks "Share → Yappy" in any iOS app, the Share Extension
// queues entries into the App Group's UserDefaults and re-opens the main app
// via the `yappy://` URL scheme. mobile::pickup_shared_payload on the Rust
// side drains the queue and emits an `ios_shared_payload` event.
//
// Each payload is one entry per line:
//   url:https://example.com/article
//   text:any selected text the user shared
//
// For URLs we fetch the HTML through the Tauri HTTP plugin (which bypasses
// WKWebView's CORS rules) and run defuddle on it inside a hidden iframe to
// extract the article body. For plain text we route directly to TTS.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import { synthesizeText } from "$lib/ipc";

// ────────────────────────────────────────────────────────────────────────
// Lazy-load defuddle.js (1.3 MB minified) only when first needed. Defuddle
// ships as a UMD bundle that assigns to `window.Defuddle`.
// ────────────────────────────────────────────────────────────────────────
let defuddlePromise: Promise<any> | null = null;
async function loadDefuddle(): Promise<any> {
  if ((window as any).Defuddle) return (window as any).Defuddle;
  if (defuddlePromise) return defuddlePromise;
  defuddlePromise = (async () => {
    // The file is bundled as a Tauri resource at /resources/defuddle.js by
    // tauri.conf.json. SvelteKit serves it from /resources/ in dev too.
    const resp = await fetch("/resources/defuddle.js");
    if (!resp.ok) throw new Error(`defuddle.js fetch failed: ${resp.status}`);
    const src = await resp.text();
    // eslint-disable-next-line no-new-func
    new Function(src)();
    return (window as any).Defuddle;
  })();
  return defuddlePromise;
}

async function extractArticleFromHtml(html: string, url: string): Promise<string> {
  const Defuddle = await loadDefuddle();
  // Defuddle takes a Document, so parse the HTML in-memory.
  const doc = new DOMParser().parseFromString(html, "text/html");
  // Defuddle's API: new Defuddle(doc, options).parse() → { title, content, ... }
  const d = new Defuddle(doc, { markdown: true, url });
  const result = d.parse();
  // result.content is markdown; result.title is the article title.
  const title = (result.title || "").trim();
  const body = (result.content || "").trim();
  if (!body) throw new Error("defuddle returned no content");
  return title ? `${title}\n\n${body}` : body;
}

async function fetchHtml(url: string): Promise<string> {
  // Tauri HTTP plugin requests run from Rust → no CORS gate.
  const resp = await tauriFetch(url, {
    method: "GET",
    headers: {
      // Pretend to be a desktop browser so paywall-by-UA sites give us
      // article body instead of the mobile/AMP variant.
      "User-Agent":
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_4) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15",
      Accept: "text/html,application/xhtml+xml",
    },
  });
  if (!resp.ok) throw new Error(`fetch ${url} → ${resp.status}`);
  return await resp.text();
}

// One payload line — "url:<...>" or "text:<...>" — handled.
async function handleOne(line: string): Promise<void> {
  if (line.startsWith("url:")) {
    const url = line.slice(4);
    console.log("[shareIntake] URL share:", url);
    try {
      const html = await fetchHtml(url);
      const article = await extractArticleFromHtml(html, url);
      console.log(`[shareIntake] defuddle extracted ${article.length} chars`);
      await synthesizeText(article);
    } catch (e) {
      console.error("[shareIntake] URL handling failed:", e);
      // Fall back to reading the URL itself so the user at least hears
      // *something* — better than silent failure.
      await synthesizeText(`couldn't extract the article. shared URL: ${url}`);
    }
    return;
  }
  if (line.startsWith("text:")) {
    const text = line.slice(5).trim();
    if (text) {
      console.log(`[shareIntake] text share: ${text.length} chars`);
      await synthesizeText(text);
    }
    return;
  }
  console.warn("[shareIntake] unknown payload prefix:", line.slice(0, 30));
}

let unlisten: UnlistenFn | null = null;

/// Start listening for Share-Sheet payloads. Call once at app boot.
/// Safe to call multiple times — re-installing replaces the previous listener.
export async function startShareIntake(): Promise<void> {
  if (unlisten) {
    unlisten();
    unlisten = null;
  }
  unlisten = await listen<string>("ios_shared_payload", async (ev) => {
    const payload = ev.payload || "";
    for (const line of payload.split("\n")) {
      const trimmed = line.trim();
      if (!trimmed) continue;
      try {
        await handleOne(trimmed);
      } catch (e) {
        console.error("[shareIntake] failed:", e);
      }
    }
  });
}
