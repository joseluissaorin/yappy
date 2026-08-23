// Holds the document the iOS in-page reader (`/read`) is showing. The home
// loads a document (read_document_cmd) into here, then navigates to /read.
// Same-window in-memory hand-off (iOS is single-window).
import type { DocumentLoaded } from "$lib/ipc";

export const reader = $state<{ doc: DocumentLoaded | null }>({ doc: null });
