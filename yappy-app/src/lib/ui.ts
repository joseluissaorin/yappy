// Tiny cross-component UI helpers. The error toast lives once in the (app)
// layout; any page reports an error by dispatching this window event so we don't
// duplicate toast markup/state per route.

export function notifyError(msg: string) {
  if (typeof window !== "undefined") {
    window.dispatchEvent(new CustomEvent("yappy:error", { detail: msg }));
  }
}
