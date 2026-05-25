import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Gender = "Male" | "Female";

export interface Voice {
  name: string;
  id: string;
  gender: Gender;
  description: string;
  tags: string[];
}

export type Quality = "fast" | "balanced" | "best";

export type PlayerPositionPreset = "top-left" | "top-center" | "top-right" | "bottom-left" | "bottom-center" | "bottom-right" | "custom";
export type PlayerTheme = "cream" | "dark" | "translucent";
export type AppTheme = "cream" | "dark" | "system";
export type OcrEngine = "auto" | "applevision" | "paddle";

export interface Settings {
  voice: string;
  voice_overrides: Record<string, string>;
  speed: number;
  volume: number;
  silence_secs: number;
  default_lang: string;
  quality: Quality;
  hotkey_read_now: string;
  hotkey_pause_resume: string;
  hotkey_read_clipboard: string;
  auto_lang_detect: boolean;
  save_history: boolean;
  history_max: number;
  successful_reads: number;
  launch_at_login: boolean;
  notify_on_done: boolean;
  sound_effects: boolean;
  doc_chunk_kb_limit: number;
  ocr_engine: OcrEngine;
  ocr_languages: string[];
  model_ready: boolean;
  first_launch_done: boolean;
  app_theme: AppTheme;
  karaoke_in_player: boolean;
  player_position_preset: PlayerPositionPreset;
  player_position: [number, number] | null;
  player_pinned: boolean;
  player_compact: boolean;
  player_theme: PlayerTheme;
  player_opacity: number;
  player_autohide_secs: number;
  player_show_source: boolean;
  player_show_waves: boolean;
  player_size: string;
}

export interface Credit {
  name: string;
  kind: string;
  version: string;
  license: string;
  url: string;
  note: string;
}
export interface LicenseDoc { name: string; url: string; text: string; }

export type CaptureSource =
  | { kind: "selection"; app_name?: string | null }
  | { kind: "active_document"; app_name: string; doc_kind: string }
  | { kind: "webpage"; app_name: string; url?: string | null; title?: string | null }
  | { kind: "ocr"; app_name?: string | null }
  | { kind: "manual" }
  | { kind: "clipboard" }
  | { kind: "file"; path: string; extension: string }
  | { kind: "history" };

export interface CaptureInfo {
  source?: CaptureSource;
  len?: number;
  preview?: string;
  // legacy file-payload (kind + path + extension shipped flat)
  kind?: string;
  path?: string;
  extension?: string;
}

export interface DownloadProgress {
  file: string;
  bytes_done: number;
  bytes_total: number;
  stage: "start" | "downloading" | "done" | string;
  overall_done: number;
  overall_total: number;
}

export interface PlaybackSnapshot {
  playing: boolean;
  paused: boolean;
  current_text: string;
  /// Chunk index (across all sentence-splits). Use for chunk-level karaoke.
  current_index: number;
  /// Paragraph index in the input. Use for paragraph-level karaoke.
  current_paragraph_index: number;
  total: number;
  total_paragraphs: number;
  elapsed_secs: number;
  duration_secs: number;
  volume: number;
  output_sample_rate: number;
}

export interface CaptureDiagnostics {
  front_app: string | null;
  selection_preview: string;
  active_doc_preview: string;
  clipboard_preview: string;
}

export interface HistoryEntry {
  id: string;
  started_at: number;
  source: string;
  app_name: string | null;
  voice: string;
  lang: string;
  text: string;
  duration_secs: number;
}

export interface History {
  entries: HistoryEntry[];
}

// --- Commands ---
export const listVoices = (): Promise<Voice[]> => invoke("list_voices");
export const getSettings = (): Promise<Settings> => invoke("get_settings");
export const setSettings = (settings: Settings): Promise<void> => invoke("set_settings", { settings });
export const setSpeed = (speed: number): Promise<void> => invoke("set_speed_cmd", { speed });
export const setVoice = (voice: string): Promise<void> => invoke("set_voice_cmd", { voice });
export const setVoiceOverride = (lang: string, voice: string | null): Promise<void> =>
  invoke("set_voice_override_cmd", { lang, voice });
export const setDefaultLang = (lang: string): Promise<void> => invoke("set_default_lang_cmd", { lang });
export const setQuality = (quality: Quality): Promise<void> => invoke("set_quality_cmd", { quality });
export const setVolume = (volume: number): Promise<void> => invoke("set_volume_cmd", { volume });
export const setSilence = (silence_secs: number): Promise<void> => invoke("set_silence_cmd", { silenceSecs: silence_secs });
export const skip = (delta_secs: number): Promise<void> => invoke("skip_cmd", { deltaSecs: delta_secs });
export const stopPlayback = (): Promise<void> => invoke("stop_playback_cmd");
export const togglePause = (): Promise<void> => invoke("toggle_pause_cmd");
export const readNow = (): Promise<void> => invoke("trigger_read_now_cmd");
export const readClipboard = (): Promise<void> => invoke("read_clipboard_cmd");
export const synthesizeText = (text: string): Promise<void> => invoke("synthesize_text", { text });
export const sampleVoice = (
  voice: string,
  lang?: string,
  sample_text?: string,
): Promise<void> => invoke("sample_voice", { voice, lang: lang ?? null, sampleText: sample_text ?? null });
export const isModelReady = (): Promise<boolean> => invoke("is_model_ready");
export const downloadModel = (): Promise<void> => invoke("download_model_cmd");
export const openMain = (): Promise<void> => invoke("open_main_window");
export const openPlayer = (): Promise<void> => invoke("open_player_window");
export const requestMacosPermissions = (): Promise<void> => invoke("request_macos_permissions");
export const captureDiagnostics = (): Promise<CaptureDiagnostics> => invoke("capture_diagnostics");

export const getHistory = (): Promise<History> => invoke("get_history");
export const clearHistory = (): Promise<void> => invoke("clear_history_cmd");
export const replayHistory = (id: string): Promise<void> => invoke("replay_history_cmd", { id });
export const saveCurrentAudio = (path: string): Promise<void> => invoke("save_current_audio_cmd", { path });
export const setHotkey = (action: "read_now" | "pause_resume" | "read_clipboard", combo: string): Promise<void> =>
  invoke("set_hotkey_cmd", { action, combo });
export const setPlayerPosition = (x: number | null, y: number | null): Promise<void> =>
  invoke("set_player_position_cmd", { x, y });
/// Open a document. If `targetWindow` is provided, the file's content replaces
/// whatever is in that window (swap behaviour from the doc window's "swap" button).
/// If absent and the caller is the main window, a brand-new document window is created.
export const readFile = (path: string, targetWindow?: string): Promise<void> =>
  invoke("read_file_cmd", { path, targetWindow });

// Document reader (separate window, full-screen reading view).
export interface DocumentLoaded {
  path: string;
  filename: string;
  extension: string;
  paragraphs: string[];
  char_count: number;
  /// True while the backend is still parsing this file. The frontend
  /// uses this to show a "parsing…" state instead of empty editor.
  loading?: boolean;
  /// Reading-rhythm hints from markdown structure parsing.
  /// Parallel arrays — index `i` describes `paragraphs[i]`.
  /// `paragraph_pauses[i]` = default seconds to pause BEFORE this paragraph.
  /// `paragraph_speed_mult[i]` = relative speed (1.0 = global, <1 = slower).
  /// `paragraph_kinds[i]` = "paragraph" | "heading1" | "heading2" | "heading3" | "list" | "hr" | "quote" | "code".
  paragraph_pauses?: number[];
  paragraph_speed_mult?: number[];
  paragraph_kinds?: string[];
}
export function onDocumentLoaded(cb: (d: DocumentLoaded) => void): Promise<UnlistenFn> {
  return listen<DocumentLoaded>("document_loaded", (ev) => cb(ev.payload));
}
export function onDocumentError(cb: (p: { filename: string; error: string }) => void): Promise<UnlistenFn> {
  return listen("document_error", (ev: any) => cb(ev.payload));
}
export const readDocumentParagraphs = (
  paragraphs: string[],
  fromIndex: number,
  voiceOverride?: string,
  speedOverride?: number,
): Promise<void> =>
  invoke("read_document_paragraphs_cmd", {
    paragraphs,
    fromIndex,
    voiceOverride: voiceOverride ?? null,
    speedOverride: speedOverride ?? null,
  });
export const getCurrentDocument = (): Promise<DocumentLoaded | null> =>
  invoke("get_current_document_cmd");
export const documentWindowReady = (): Promise<void> => invoke("document_window_ready_cmd");
export const clearCurrentDocument = (): Promise<void> => invoke("clear_current_document_cmd");
export const saveProject = (docPath: string, projectJson: string): Promise<void> =>
  invoke("save_project_cmd", { docPath, projectJson });
export const loadProject = (docPath: string): Promise<string | null> =>
  invoke("load_project_cmd", { docPath });

// Audiobook export: render all paragraphs end-to-end as one audio file.
// outputPath extension picks the format: `.m4b` → M4B audiobook with chapters,
// anything else → 16-bit WAV. `chapter_title` on a paragraph marks the start
// of a chapter in the m4b output (ignored for .wav).
export interface ParagraphSpec {
  text: string;
  voice?: string | null;
  speed?: number | null;
  pause_before?: number | null;
  chapter_title?: string | null;
}
export interface AudiobookMeta {
  title?: string | null;
  author?: string | null;
  album?: string | null;
}
export const renderAudiobook = (
  paragraphs: ParagraphSpec[],
  outputPath: string,
  metadata?: AudiobookMeta,
): Promise<void> =>
  invoke("render_audiobook_cmd", { paragraphs, outputPath, metadata: metadata ?? null });
export function onAudiobookRenderProgress(
  cb: (p: { index: number; total: number; stage: string }) => void,
): Promise<UnlistenFn> {
  return listen("audiobook_render_progress", (ev: any) => cb(ev.payload));
}
export function onAudiobookRenderDone(cb: (p: { path: string; samples: number; sample_rate: number }) => void): Promise<UnlistenFn> {
  return listen("audiobook_render_done", (ev: any) => cb(ev.payload));
}

export const openBrowserExtensions = (browser: string): Promise<void> =>
  invoke("open_browser_extensions_cmd", { browser });
export const getExtensionPath = (): Promise<string> => invoke("get_extension_path_cmd");
export const revealExtensionFolder = (): Promise<void> => invoke("reveal_extension_folder_cmd");
export const revealLogFile = (): Promise<string> => invoke("reveal_log_file_cmd");
export const getLogPath = (): Promise<string> => invoke("get_log_path_cmd");
export const tailLog = (maxKb?: number): Promise<string> => invoke("tail_log_cmd", { maxKb });

/// Forward a frontend message into the backend's tracing log. Used to capture
/// what's happening inside the webview when DevTools isn't available.
export async function logToBackend(level: "info" | "warn" | "error" | "debug", source: string, message: string) {
  try {
    await invoke("log_frontend_cmd", { level, source, message });
  } catch {
    // Ignore — logging never blocks the UI.
  }
}
export const setLaunchAtLogin = (enabled: boolean): Promise<void> =>
  invoke("set_launch_at_login_cmd", { enabled });

// Browser extension bridge.
export interface BridgeConnection {
  browser: string;
  connected_at: number;
  last_seen: number;
}
export interface BridgeStatus {
  enabled: boolean;
  token: string;
  port: number;
  connections: BridgeConnection[];
}
export const bridgeStatus = (): Promise<BridgeStatus> => invoke("bridge_status");
export const bridgeRegenerateToken = (): Promise<string> => invoke("bridge_regenerate_token_cmd");
export const bridgeClearPairing = (): Promise<void> => invoke("bridge_clear_pairing_cmd");
export const setBridgeEnabled = (enabled: boolean): Promise<void> =>
  invoke("set_bridge_enabled_cmd", { enabled });
export function onBridgePaired(cb: (browser: string) => void): Promise<UnlistenFn> {
  return listen<string>("bridge_paired", (ev) => cb(ev.payload));
}
export function onBridgeDisconnected(cb: (browser: string) => void): Promise<UnlistenFn> {
  return listen<string>("bridge_disconnected", (ev) => cb(ev.payload));
}
export function onBridgeTokenChanged(cb: (token: string) => void): Promise<UnlistenFn> {
  return listen<string>("bridge_token_changed", (ev) => cb(ev.payload));
}

export const listCredits = (): Promise<Credit[]> => invoke("list_credits");
export const listLicenses = (): Promise<LicenseDoc[]> => invoke("list_licenses");

export const setPlayerPreset = (preset: PlayerPositionPreset): Promise<void> => invoke("set_player_preset_cmd", { preset });
export const setPlayerTheme = (theme: PlayerTheme): Promise<void> => invoke("set_player_theme_cmd", { theme });
export const setAppTheme = (theme: AppTheme): Promise<void> => invoke("set_app_theme_cmd", { theme });
export const setPlayerSize = (size: string): Promise<void> => invoke("set_player_size_cmd", { size });
export const setOcrEngine = (engine: OcrEngine): Promise<void> => invoke("set_ocr_engine_cmd", { engine });
export const resetSettings = (): Promise<Settings> => invoke("reset_settings_cmd");
export const exportSettings = (): Promise<string> => invoke("export_settings_cmd");
export const importSettings = (json: string): Promise<Settings> => invoke("import_settings_cmd", { json });

// --- Events ---
export function onModelDownload(cb: (p: DownloadProgress) => void): Promise<UnlistenFn> {
  return listen<DownloadProgress>("model_download", (ev) => cb(ev.payload));
}
export function onPlaybackState(cb: (s: PlaybackSnapshot) => void): Promise<UnlistenFn> {
  return listen<PlaybackSnapshot>("playback_state", (ev) => cb(ev.payload));
}
export function onChunkSynthesized(cb: (p: { index: number; total: number; text: string; lang: string }) => void): Promise<UnlistenFn> {
  return listen("chunk_synthesized", (ev: any) => cb(ev.payload));
}
export function onPlaybackStarting(cb: (p: { text_preview: string; source: string; base_paragraph_index?: number }) => void): Promise<UnlistenFn> {
  return listen("playback_starting", (ev: any) => cb(ev.payload));
}
export function onCaptureEmpty(cb: () => void): Promise<UnlistenFn> {
  return listen("capture_empty", () => cb());
}
export function onCaptureInfo(cb: (info: CaptureInfo) => void): Promise<UnlistenFn> {
  return listen("capture_info", (ev: any) => cb(ev.payload));
}
export function onCaptureProgress(cb: (stage: string) => void): Promise<UnlistenFn> {
  return listen<string>("capture_progress", (ev) => cb(ev.payload));
}
export function onModelMissing(cb: () => void): Promise<UnlistenFn> {
  return listen("model_missing", () => cb());
}
export function onSynthError(cb: (msg: string) => void): Promise<UnlistenFn> {
  return listen<string>("synth_error", (ev) => cb(ev.payload));
}
export function onNav(cb: (page: string) => void): Promise<UnlistenFn> {
  return listen<string>("nav", (ev) => cb(ev.payload));
}
export function onFirstRead(cb: () => void): Promise<UnlistenFn> {
  return listen("first_read", () => cb());
}

export const LANGUAGES: { code: string; label: string; flag?: string }[] = [
  { code: "en", label: "English", flag: "🇬🇧" },
  { code: "es", label: "Español", flag: "🇪🇸" },
  { code: "fr", label: "Français", flag: "🇫🇷" },
  { code: "de", label: "Deutsch", flag: "🇩🇪" },
  { code: "it", label: "Italiano", flag: "🇮🇹" },
  { code: "pt", label: "Português", flag: "🇵🇹" },
  { code: "nl", label: "Nederlands", flag: "🇳🇱" },
  { code: "pl", label: "Polski", flag: "🇵🇱" },
  { code: "ro", label: "Română", flag: "🇷🇴" },
  { code: "sv", label: "Svenska", flag: "🇸🇪" },
  { code: "da", label: "Dansk", flag: "🇩🇰" },
  { code: "fi", label: "Suomi", flag: "🇫🇮" },
  { code: "et", label: "Eesti", flag: "🇪🇪" },
  { code: "lt", label: "Lietuvių", flag: "🇱🇹" },
  { code: "lv", label: "Latviešu", flag: "🇱🇻" },
  { code: "hr", label: "Hrvatski", flag: "🇭🇷" },
  { code: "sl", label: "Slovenščina", flag: "🇸🇮" },
  { code: "sk", label: "Slovenčina", flag: "🇸🇰" },
  { code: "cs", label: "Čeština", flag: "🇨🇿" },
  { code: "hu", label: "Magyar", flag: "🇭🇺" },
  { code: "el", label: "Ελληνικά", flag: "🇬🇷" },
  { code: "bg", label: "Български", flag: "🇧🇬" },
  { code: "uk", label: "Українська", flag: "🇺🇦" },
  { code: "ru", label: "Русский", flag: "🇷🇺" },
  { code: "tr", label: "Türkçe", flag: "🇹🇷" },
  { code: "ar", label: "العربية", flag: "🇸🇦" },
  { code: "hi", label: "हिन्दी", flag: "🇮🇳" },
  { code: "id", label: "Bahasa Indonesia", flag: "🇮🇩" },
  { code: "vi", label: "Tiếng Việt", flag: "🇻🇳" },
  { code: "ko", label: "한국어", flag: "🇰🇷" },
  { code: "ja", label: "日本語", flag: "🇯🇵" },
  { code: "na", label: "Auto-detect" },
];

export function langLabel(code: string): string {
  return LANGUAGES.find((l) => l.code === code)?.label ?? code;
}
