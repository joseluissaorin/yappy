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
import { invoke } from "@tauri-apps/api/core";
import { goto } from "$app/navigation";
import { fetch as tauriFetch } from "@tauri-apps/plugin-http";
import {
  synthesizeText,
  saveTranscript,
  readTextAsDocument,
  readDocumentParagraphs,
  colaAgregarUrl,
  colaAgregarWeb,
  colaAgregarTexto,
  colaAgregarArchivo,
  colaAgregarAudio,
  colaListar,
  onColaActualizada,
  readDocument,
} from "$lib/ipc";
import { reader } from "$lib/readerStore.svelte";
import { soltarLlegada } from "$lib/llegada.svelte";

// Load extracted/shared text into the immersive reader as a document (so it
// shows with sections + maintains proper playback state / mini-player), then
// start reading it aloud from the top. Falls back to a blind synth if loading
// the document fails for any reason.
async function openInReaderAndRead(text: string, title: string): Promise<void> {
  try {
    const doc = await readTextAsDocument(text, title);
    reader.doc = doc;
    await goto("/read");
    await readDocumentParagraphs(doc.paragraphs, 0, undefined, undefined, undefined, {
      docPath: doc.path,
      titulo: title,
    });
  } catch (e) {
    console.error("[shareIntake] openInReader failed, falling back to blind synth:", e);
    await synthesizeText(text);
  }
}

// Derive a short, filename-safe title from the first heading/line of markdown.
function titleFromArticle(article: string): string {
  const first = article.split("\n").find((l) => l.trim().length > 0) ?? "Shared article";
  return first.replace(/^#+\s*/, "").trim().slice(0, 60) || "Shared article";
}

// ────────────────────────────────────────────────────────────────────────
// Lazy-load defuddle.js (1.3 MB minified) only when first needed. Defuddle
// ships as a UMD bundle that assigns to `window.Defuddle`.
// ────────────────────────────────────────────────────────────────────────
let defuddlePromise: Promise<any> | null = null;
async function loadDefuddle(): Promise<any> {
  if ((window as any).Defuddle) return (window as any).Defuddle;
  if (defuddlePromise) return defuddlePromise;
  defuddlePromise = (async () => {
    // Served as a SvelteKit static asset at /defuddle.js (static/defuddle.js).
    // NOTE: it is NOT fetchable from /resources/ — that path is a Tauri *bundle
    // resource* (not exposed to the webview), so fetching it 404s on device and
    // article extraction silently falls back. Keep this pointing at the static
    // copy. (static/defuddle.js is kept in sync with resources/defuddle.js.)
    const resp = await fetch("/defuddle.js");
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

// El pitch entero cabe en un gesto: estás leyendo un artículo, te tienes
// que poner a cocinar, compartes con Yappy y EMPIEZA A LEERSE SOLO. Por eso
// cada línea del payload entra en LA COLA (la extracción ocurre en Rust,
// con estado visible) y lo recién compartido queda apuntado para
// reproducirse en cuanto esté listo, sin más toques.
const reproducirAlLlegar = new Set<string>();
let vigilanciaColá: UnlistenFn | null = null;

async function vigilarAutoplay(): Promise<void> {
  if (vigilanciaColá) return;
  vigilanciaColá = await onColaActualizada(async () => {
    if (reproducirAlLlegar.size === 0) return;
    try {
      const items = await colaListar();
      for (const item of items) {
        if (!reproducirAlLlegar.has(item.id)) continue;
        if (item.estado === "error") {
          reproducirAlLlegar.delete(item.id);
          continue;
        }
        if (item.estado === "listo" && item.ruta) {
          reproducirAlLlegar.delete(item.id);
          const doc = await readDocument(item.ruta);
          doc.filename = item.titulo;
          reader.doc = doc;
          soltarLlegada(item.titulo);
          await goto("/read");
          await readDocumentParagraphs(doc.paragraphs, 0, undefined, undefined, undefined, {
            docPath: item.ruta,
            titulo: item.titulo,
          });
          break;
        }
      }
    } catch (e) {
      console.error("[shareIntake] autoplay:", e);
    }
  });
}

async function handleOne(line: string): Promise<void> {
  let encolado = false;
  if (line.startsWith("url:")) {
    const url = line.slice(4).trim();
    if (url) {
      const item = await colaAgregarUrl(url);
      reproducirAlLlegar.add(item.id);
      await vigilarAutoplay();
      encolado = true;
    }
  } else if (line.startsWith("web:")) {
    // Página viva de Safari: el HTML con la sesión del usuario ya está en
    // el App Group; Rust extrae sin descargar (los muros no existen).
    const ruta = line.slice(4).trim();
    if (ruta) {
      const item = await colaAgregarWeb(ruta);
      reproducirAlLlegar.add(item.id);
      await vigilarAutoplay();
      encolado = true;
    }
  } else if (line.startsWith("text:")) {
    const text = line.slice(5).trim();
    if (text) {
      const item = await colaAgregarTexto(text);
      encolado = true;
      if (text.length <= 280) {
        synthesizeText(text).catch(() => {});
      } else if (item.ruta) {
        // Texto largo compartido: al lector, y sonando, con SU título (el
        // nombre de fichero de la cola son números).
        try {
          const doc = await readDocument(item.ruta);
          doc.filename = item.titulo;
          reader.doc = doc;
          await goto("/read");
          await readDocumentParagraphs(doc.paragraphs, 0, undefined, undefined, undefined, {
            docPath: item.ruta,
            titulo: item.titulo,
          });
          return;
        } catch {}
      }
    }
  } else if (line.startsWith("transcript:")) {
    const text = line.slice("transcript:".length).trim();
    if (text) {
      await colaAgregarTexto(text, "Transcripción");
      encolado = true;
      try {
        const entry = await saveTranscript(text, "Shared");
        window.dispatchEvent(new CustomEvent("yappy:transcript", { detail: entry }));
      } catch {}
    }
  } else if (line.startsWith("audio:")) {
    const path = line.slice("audio:".length).trim();
    if (path) {
      await colaAgregarAudio(path);
      encolado = true;
    }
  } else if (line.startsWith("file:")) {
    // PDF, EPUB, DOCX…: la extensión los copió al App Group. Un archivo
    // queda «listo» al instante, así que suena directamente.
    const path = line.slice("file:".length).trim();
    if (path) {
      const item = await colaAgregarArchivo(path);
      encolado = true;
      if (item.ruta) {
        try {
          const doc = await readDocument(item.ruta);
          // El fichero copiado se llama por su id numérico: el título
          // humano vive en la pieza de la cola. Sin esto, el lector y la
          // pantalla de bloqueo enseñaban «1724…-0001».
          doc.filename = item.titulo;
          reader.doc = doc;
          await goto("/read");
          await readDocumentParagraphs(doc.paragraphs, 0, undefined, undefined, undefined, {
            docPath: item.ruta,
            titulo: item.titulo,
          });
          return;
        } catch (e) {
          console.error("[shareIntake] abrir archivo compartido:", e);
        }
      }
    }
  } else if (line.startsWith("accion:")) {
    // Los App Intents (Siri, Atajos, botón de acción) encolan acciones por
    // este mismo canal.
    const accion = line.slice("accion:".length).trim();
    if (accion === "read-clipboard") {
      await invoke("read_clipboard_cmd").catch(() => {});
    } else if (accion === "resume") {
      await invoke("toggle_pause_cmd").catch(() => {});
    }
  } else {
    console.warn("[shareIntake] unknown payload prefix:", line.slice(0, 30));
  }
  if (encolado) {
    goto("/escuchar").catch(() => {});
  }
}

// Process a newline-separated payload string (one share entry per line).
async function handlePayload(payload: string): Promise<void> {
  for (const line of (payload || "").split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    try {
      await handleOne(trimmed);
    } catch (e) {
      console.error("[shareIntake] failed:", e);
    }
  }
}

// Pull any pending Share-Sheet payloads from the App Group queue. We do this
// (rather than only listening for the Rust-emitted event) because on a COLD
// launch the backend would emit before this webview registered its listener,
// losing the shared item. Pulling when WE'RE ready — on mount and on every
// foreground — guarantees we never miss one. No-op off iOS (returns null).
let draining = false;
export async function drainPending(): Promise<void> {
  if (draining) return;
  draining = true;
  try {
    const payload = await invoke<string | null>("drain_shared_payloads_cmd");
    if (payload) {
      console.log("[shareIntake] drained pending payload(s)");
      await handlePayload(payload);
    }
  } catch (e) {
    console.error("[shareIntake] drain failed:", e);
  } finally {
    draining = false;
  }
}

let unlisten: UnlistenFn | null = null;
let visibilityHandler: (() => void) | null = null;
let intervaloDrain: ReturnType<typeof setInterval> | null = null;

/// Start listening for Share-Sheet payloads. Call once at app boot.
/// Safe to call multiple times — re-installing replaces the previous listener.
export async function startShareIntake(): Promise<void> {
  if (unlisten) {
    unlisten();
    unlisten = null;
  }
  // Keep the event path too (the backend may still emit while we're alive).
  unlisten = await listen<string>("ios_shared_payload", async (ev) => {
    await handlePayload(ev.payload || "");
  });

  // Cold-launch: pull whatever's already queued now that we're ready.
  await drainPending();

  // Warm reopen: when the Share Extension re-opens the app (yappy://shared)
  // while it's already running, the webview becomes visible again — re-pull.
  if (typeof document !== "undefined") {
    if (visibilityHandler) document.removeEventListener("visibilitychange", visibilityHandler);
    visibilityHandler = () => {
      if (document.visibilityState === "visible") drainPending();
    };
    document.addEventListener("visibilitychange", visibilityHandler);
  }

  // Android entrega los intents con la app YA visible (onNewIntent), sin
  // cambio de visibilidad que dispare el drenaje: un pulso barato lo cubre
  // (leer un fichero pequeño cada pocos segundos).
  if (!intervaloDrain) {
    intervaloDrain = setInterval(() => drainPending(), 6000);
  }

  // Dev helper: expose handleOne on window so we can drive the defuddle
  // path from Safari Web Inspector / WKWebView console for iOS testing
  // BEFORE the App Group entitlement is honored. Call from devtools:
  //   await window.__yappyHandleOne("url:https://example.com/article")
  if (typeof window !== "undefined") {
    (window as any).__yappyHandleOne = handleOne;
  }
}
