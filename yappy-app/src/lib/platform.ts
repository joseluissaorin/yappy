// Platform detection. Resolved once at startup via @tauri-apps/plugin-os.
// Yappy's UI was originally designed for macOS desktop; on iOS we hide the
// chunks that don't apply (global hotkey configuration, browser-extension
// pairing, autostart, tray-menu shortcuts) and swap copy that mentions
// "your mac".
//
// Usage:
//   import { isIOS, platformName, ready } from "$lib/platform";
//   {#await ready then _}
//     {#if !$isIOS}<DesktopOnlyThing />{/if}
//   {/await}
//
// `platformName` is the raw tauri OsType string ("macos"/"linux"/"windows"/
// "ios"/"android"). `isIOS` is the convenience derived store.

import { writable, derived, type Readable } from "svelte/store";
import { type as osType } from "@tauri-apps/plugin-os";

export const platformName = writable<string>("unknown");
export const isIOS: Readable<boolean> = derived(platformName, ($p) => $p === "ios");
export const isMobile: Readable<boolean> = derived(platformName, ($p) => $p === "ios" || $p === "android");

// Resolve the platform once at startup. Components that need to make
// decisions based on platform should `await ready` before rendering
// platform-dependent branches.
export const ready: Promise<string> = (async () => {
  try {
    const t = await osType();
    platformName.set(t);
    return t;
  } catch (e) {
    console.warn("[platform] osType() failed — defaulting to macos:", e);
    platformName.set("macos");
    return "macos";
  }
})();
