// Centralized page navigation for the multi-page main window.
//
// Lightweight pages are client-side routes inside the main window. Transcription
// is hybrid: on desktop it opens its own native window (like the editor/player);
// on iOS (single-window) it's just another client route.

import { get } from "svelte/store";
import { goto } from "$app/navigation";
import { isIOS } from "$lib/platform";
import { openTranscribeWindow } from "$lib/ipc";

export type Section =
  | "home"
  | "voices"
  | "transcribe"
  | "library"
  | "preferences"
  | "history"
  | "diagnostics";

export const ROUTES: Record<Section, string> = {
  home: "/",
  voices: "/voices",
  transcribe: "/transcribe",
  library: "/library",
  preferences: "/preferences",
  history: "/history",
  diagnostics: "/diagnostics",
};

/// Navigate to a section. `opts.path` hands an audio file to transcription.
export async function goPage(section: Section, opts?: { path?: string }) {
  if (section === "transcribe" && !get(isIOS)) {
    // Desktop: dedicated transcription window.
    await openTranscribeWindow(opts?.path);
    return;
  }
  await goto(ROUTES[section]);
}

/// Which nav section the current URL belongs to (for the active-tab highlight).
export function sectionForPath(pathname: string): Section {
  if (pathname === "/" || pathname === "") return "home";
  const seg = pathname.split("/").filter(Boolean)[0] ?? "home";
  return (Object.keys(ROUTES) as Section[]).find((s) => ROUTES[s] === `/${seg}`) ?? "home";
}
