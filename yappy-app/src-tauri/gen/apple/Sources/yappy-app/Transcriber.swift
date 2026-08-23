// On-device speech-to-text for Apple platforms via FluidAudio's CoreML Parakeet
// TDT (Neural Engine, ~66 MB RAM, multilingual). Shared by the main app (over
// the C-ABI below, called from `mobile.rs`) AND the Share Extension (which calls
// `YappyTranscriber.shared` directly to transcribe a WhatsApp voice note in-place
// without leaving the host app).
//
// The CoreML model lives in the App Group container so the main app downloads it
// once and the extension reuses it — the extension never downloads (it's
// short-lived + memory-bounded); if the model is absent it tells the user to open
// Yappy first.

import Foundation
import FluidAudio

private let APP_GROUP = "group.com.joseluissaorin.yappy"

@available(iOS 17.0, macOS 14.0, *)
final class YappyTranscriber {
    static let shared = YappyTranscriber()
    private var manager: AsrManager?

    /// Shared model directory inside the App Group, so the main app + the Share
    /// Extension use one copy. Falls back to FluidAudio's default cache if the
    /// App Group container is unavailable (e.g. entitlement missing in dev).
    static func modelsDir() -> URL {
        if let c = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: APP_GROUP) {
            return c.appendingPathComponent("asr-models", isDirectory: true)
        }
        return AsrModels.defaultCacheDirectory(for: .v3)
    }

    /// Whether the model is already downloaded (no network / load needed to ask).
    var isReady: Bool { AsrModels.modelsExist(at: Self.modelsDir()) }

    /// Load the model from disk into memory. Does NOT download.
    func ensureLoaded() async throws {
        if manager != nil { return }
        let models = try await AsrModels.load(from: Self.modelsDir(), version: .v3)
        let m = AsrManager(config: .default)
        try await m.loadModels(models)
        manager = m
    }

    /// Download + load the model into the shared App Group dir. Main-app only.
    func download() async throws {
        let dir = Self.modelsDir()
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let models = try await AsrModels.downloadAndLoad(to: dir, version: .v3)
        let m = AsrManager(config: .default)
        try await m.loadModels(models)
        manager = m
    }

    /// Transcribe an audio file at `path`. FluidAudio decodes + resamples the
    /// file internally. Returns nil on any failure.
    func transcribe(path: String) async -> String? {
        do {
            try await ensureLoaded()
            guard let m = manager else { return nil }
            var state = try TdtDecoderState()
            let result = try await m.transcribe(URL(fileURLWithPath: path), decoderState: &state)
            return result.text
        } catch {
            NSLog("[yappy/asr] transcribe failed: \(error)")
            return nil
        }
    }
}

// ─── C ABI (consumed by mobile.rs on the main app) ──────────────────────────
// `yappy_transcribe` blocks until the async transcription completes — Rust calls
// it from a blocking task, never the main thread, so the semaphore wait is safe.

@_cdecl("yappy_transcribe")
public func yappy_transcribe(_ pathPtr: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? {
    guard let p = pathPtr else { return nil }
    let path = String(cString: p)
    guard #available(iOS 17.0, macOS 14.0, *) else { return nil }
    let sem = DispatchSemaphore(value: 0)
    var out: String?
    Task.detached {
        out = await YappyTranscriber.shared.transcribe(path: path)
        sem.signal()
    }
    sem.wait()
    if let out { return strdup(out) }
    return nil
}

@_cdecl("yappy_asr_model_ready")
public func yappy_asr_model_ready() -> Bool {
    guard #available(iOS 17.0, macOS 14.0, *) else { return false }
    return YappyTranscriber.shared.isReady
}

@_cdecl("yappy_asr_download_model")
public func yappy_asr_download_model() {
    guard #available(iOS 17.0, macOS 14.0, *) else { return }
    let sem = DispatchSemaphore(value: 0)
    Task.detached {
        try? await YappyTranscriber.shared.download()
        sem.signal()
    }
    sem.wait()
}
