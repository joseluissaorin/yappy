// Quick sanity test: load defuddle.js the same way the iOS WKWebView would
// (via `new Function(src)()` after a fetch) and verify it extracts an article.
//
// This decouples "does defuddle as integrated work" from "does the App
// Group plumbing on iOS work", since the latter is gated on user actions
// I can't take.
//
// Run: node tests/defuddle.test.mjs

import { readFileSync } from "node:fs";
import { JSDOM } from "jsdom";

const SAMPLE_HTML = `
<!DOCTYPE html>
<html>
<head><title>Sample Article About TTS</title></head>
<body>
  <nav class="site-nav">Home · Articles · About</nav>
  <header><h1>How text-to-speech engines work</h1></header>
  <main>
    <article>
      <p>Text-to-speech systems convert written language into audible speech.
      Modern engines use neural networks trained on hours of recorded human
      voices, learning the relationship between graphemes and phonemes, then
      generating waveforms via vocoders.</p>
      <p>The two biggest quality differentiators in 2026 are model size
      (parameter count) and language coverage — most open models do English
      well, fewer do Spanish, fewer still do tonal languages like Mandarin.</p>
    </article>
  </main>
  <aside class="related-articles">
    <h3>You might also like</h3>
    <ul><li>How OCR works</li><li>How encoders work</li></ul>
  </aside>
  <footer>© 2026 Some Blog · Privacy · Terms</footer>
</body>
</html>
`;

const defuddleSrc = readFileSync("resources/defuddle.js", "utf-8");

// Set up a JSDOM that exposes the globals defuddle expects (window, document).
const dom = new JSDOM(SAMPLE_HTML, { url: "https://example.com/article" });
globalThis.window = dom.window;
globalThis.document = dom.window.document;
globalThis.DOMParser = dom.window.DOMParser;
globalThis.Node = dom.window.Node;

// Execute the UMD bundle — it'll attach to window.Defuddle (since we set
// `window` above, the UMD detects browser-mode and assigns there).
new Function(defuddleSrc).call(dom.window);

const Defuddle = dom.window.Defuddle;
if (!Defuddle) {
  console.error("FAIL: window.Defuddle not present after loading bundle");
  process.exit(1);
}

const d = new Defuddle(dom.window.document, { markdown: true, url: "https://example.com/article" });
const result = d.parse();

console.log("─── Defuddle result ───────────────────────────────────────");
console.log("title:  ", result.title);
console.log("byline: ", result.byline);
console.log("content (first 400 chars):");
console.log((result.content || "").slice(0, 400));
console.log("──────────────────────────────────────────────────────────");

// Assertions
const ok = (cond, msg) => { if (!cond) { console.error("FAIL:", msg); process.exit(1); } };
ok(result.content && result.content.length > 100, "content too short");
ok(result.content.includes("phonemes") || result.content.includes("vocoders"), "article body missing");
ok(!result.content.includes("Privacy · Terms"), "footer leaked into content");
ok(!result.content.includes("You might also like"), "sidebar leaked into content");
console.log("✓ defuddle correctly extracted the article and stripped chrome");
