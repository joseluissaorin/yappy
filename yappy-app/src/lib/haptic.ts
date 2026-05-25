// Haptic feedback helper. No-op outside iOS so callers can sprinkle it
// without guards.
//
// Categories follow Apple HIG:
//   - "light" / "medium" / "heavy": physical impact (button taps)
//   - "selection": picker / segmented-control change
//   - "success" / "warning" / "error": notification feedback (after an action)

import { invoke } from "@tauri-apps/api/core";
import { isIOS } from "$lib/platform";
import { get } from "svelte/store";

export type HapticKind =
  | "light" | "medium" | "heavy"
  | "selection"
  | "success" | "warning" | "error";

export function haptic(kind: HapticKind = "light"): void {
  if (!get(isIOS)) return;
  invoke("haptic_cmd", { kind }).catch(() => {});
}
