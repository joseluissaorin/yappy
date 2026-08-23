// Share Extension — Yappy's in-place transcription modal.
//
// When the user shares an **audio** message (e.g. a WhatsApp voice note) into
// Yappy, this presents a modal RIGHT OVER the host app: it transcribes the audio
// on-device (CoreML Parakeet, via the shared `YappyTranscriber`) without ever
// leaving WhatsApp, shows the text with Copy / Open-in-Yappy, and queues the
// transcript into the App Group so it also lands in Yappy's in-app history.
//
// Shared **text/URL** keeps the original lightweight behavior: queue it and open
// the main app to read it aloud.

import UIKit
import SwiftUI
import UniformTypeIdentifiers

private let APP_GROUP = "group.com.joseluissaorin.yappy"

@objc(ShareViewController)
class ShareViewController: UIViewController {

    private var finished = false

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .clear
        // Safety net: no matter what happens (a provider callback that never
        // fires, a slow copy, etc.), never leave the share sheet dimmed and
        // stuck — always tear down within a few seconds. The payload, if any,
        // is already queued by then and the main app picks it up on next open.
        DispatchQueue.main.asyncAfter(deadline: .now() + 6) { [weak self] in
            self?.close()
        }
        handleInput()
    }

    private func handleInput() {
        let items = (extensionContext?.inputItems as? [NSExtensionItem]) ?? []
        for item in items {
            for provider in (item.attachments ?? []) {
                if provider.hasItemConformingToTypeIdentifier(UTType.audio.identifier) {
                    loadAudio(provider)
                    return
                }
            }
        }
        // No audio — fall back to the legacy text/URL queue-and-open flow.
        handleTextOrUrl(items)
    }

    // MARK: - Audio → in-place transcription modal

    private func loadAudio(_ provider: NSItemProvider) {
        provider.loadItem(forTypeIdentifier: UTType.audio.identifier, options: nil) { [weak self] data, _ in
            guard let self else { return }
            // IMPORTANT: do NOT transcribe inside the extension. Share/action
            // extensions have a hard ~120 MB memory limit; loading the ~670 MB
            // Parakeet model here gets the extension jetsam-killed (symptom: the
            // modal shows "Transcribing…" forever and never returns). Instead we
            // copy the audio into the App Group container and hand off to the
            // main app — which has the full memory budget and the same on-device
            // transcribe path (see shareIntake.ts `audio:` handler).
            var sharedURL: URL?
            if let url = data as? URL {
                sharedURL = self.copyToAppGroup(url)
            } else if let raw = data as? Data {
                sharedURL = self.writeToAppGroup(raw, ext: "m4a")
            }
            DispatchQueue.main.async {
                if let sharedURL {
                    NSLog("[yappy/share] queued audio:\(sharedURL.lastPathComponent)")
                    self.persistPayload("audio:\(sharedURL.path)")
                    self.openMainAppAndClose()
                } else {
                    self.close()
                }
            }
        }
    }

    private func presentTranscription(_ audioURL: URL) {
        let model = TranscriptionModel(audioURL: audioURL,
                                       onPersist: { [weak self] text in self?.persistTranscript(text) },
                                       onOpenApp: { [weak self] in self?.openMainAppAndClose() },
                                       onClose: { [weak self] in self?.close() })
        let host = UIHostingController(rootView: TranscriptionView(model: model))
        host.modalPresentationStyle = .automatic
        present(host, animated: true) {
            model.start()
        }
    }

    // MARK: - Legacy text / URL

    private func handleTextOrUrl(_ items: [NSExtensionItem]) {
        let group = DispatchGroup()
        var captured: [String] = []
        for item in items {
            for provider in (item.attachments ?? []) {
                if provider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
                    group.enter()
                    provider.loadItem(forTypeIdentifier: UTType.url.identifier, options: nil) { data, _ in
                        if let url = data as? URL { captured.append("url:\(url.absoluteString)") }
                        group.leave()
                    }
                } else if provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
                    group.enter()
                    provider.loadItem(forTypeIdentifier: UTType.plainText.identifier, options: nil) { data, _ in
                        if let text = data as? String { captured.append("text:\(text)") }
                        group.leave()
                    }
                }
            }
        }
        group.notify(queue: .main) {
            NSLog("[yappy/share] captured \(captured.count) text/url item(s)")
            for c in captured { self.persistPayload(c) }
            self.openMainAppAndClose()
        }
    }

    // MARK: - App Group persistence

    private func persistTranscript(_ text: String) {
        persistPayload("transcript:\(text)")
    }

    private func persistPayload(_ payload: String) {
        guard let defaults = UserDefaults(suiteName: APP_GROUP) else {
            NSLog("[yappy/share] App Group UserDefaults unavailable")
            return
        }
        var queue = defaults.array(forKey: "shared_payloads") as? [[String: Any]] ?? []
        queue.append(["payload": payload, "user_title": "", "ts": Int(Date().timeIntervalSince1970 * 1000)])
        defaults.set(queue, forKey: "shared_payloads")
        NSLog("[yappy/share] persisted payload (queue now \(queue.count)): \(payload.prefix(40))")
    }

    // MARK: - Helpers

    /// Directory inside the App Group container that BOTH the extension and the
    /// main app can read. (The extension's own temporaryDirectory is NOT shared.)
    private func sharedAudioDir() -> URL? {
        guard let container = FileManager.default
            .containerURL(forSecurityApplicationGroupIdentifier: APP_GROUP) else { return nil }
        let dir = container.appendingPathComponent("shared-audio", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    private func copyToAppGroup(_ url: URL) -> URL? {
        guard let dir = sharedAudioDir() else { return copyToTemp(url) }
        let dst = dir.appendingPathComponent(UUID().uuidString)
            .appendingPathExtension(url.pathExtension.isEmpty ? "m4a" : url.pathExtension)
        try? FileManager.default.removeItem(at: dst)
        do {
            try FileManager.default.copyItem(at: url, to: dst)
            return dst
        } catch {
            NSLog("[yappy/share] copy audio to App Group failed: \(error)")
            return nil
        }
    }

    private func writeToAppGroup(_ data: Data, ext: String) -> URL? {
        guard let dir = sharedAudioDir() else { return writeTemp(data, ext: ext) }
        let dst = dir.appendingPathComponent(UUID().uuidString).appendingPathExtension(ext)
        do { try data.write(to: dst); return dst } catch {
            NSLog("[yappy/share] write audio to App Group failed: \(error)")
            return nil
        }
    }

    private func copyToTemp(_ url: URL) -> URL? {
        let dst = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
            .appendingPathExtension(url.pathExtension.isEmpty ? "m4a" : url.pathExtension)
        try? FileManager.default.removeItem(at: dst)
        do {
            try FileManager.default.copyItem(at: url, to: dst)
            return dst
        } catch {
            NSLog("[yappy/share] copy audio failed: \(error)")
            return nil
        }
    }

    private func writeTemp(_ data: Data, ext: String) -> URL? {
        let dst = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString).appendingPathExtension(ext)
        do { try data.write(to: dst); return dst } catch { return nil }
    }

    /// Open the containing app (yappy://shared), then tear down the extension.
    ///
    /// The working technique is to walk the responder chain to whoever responds
    /// to the SINGLE-argument `openURL:` selector and `perform` it. (My earlier
    /// bug: I used `open(_:options:completionHandler:)` — a 3-arg selector — via
    /// `perform(_:with:)`, which only passes one argument, so it silently did
    /// nothing. That's why the app never opened, NOT an iOS restriction.) We fall
    /// back to `NSExtensionContext.open` if nothing in the chain responds, then
    /// complete the request after a short delay so the launch can take effect.
    private func openMainAppAndClose() {
        guard let url = URL(string: "yappy://shared") else { close(); return }
        let openSel = NSSelectorFromString("openURL:")
        var opened = false
        var responder: UIResponder? = self
        while let r = responder {
            if r.responds(to: openSel) {
                r.perform(openSel, with: url)
                opened = true
                NSLog("[yappy/share] opened main app via openURL: on \(type(of: r))")
                break
            }
            responder = r.next
        }
        if !opened {
            NSLog("[yappy/share] no openURL: responder; trying extensionContext.open")
            extensionContext?.open(url, completionHandler: nil)
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.4) { self.close() }
    }

    private func close() {
        // Idempotent — the watchdog and the normal completion can both fire.
        guard !finished else { return }
        finished = true
        extensionContext?.completeRequest(returningItems: [], completionHandler: nil)
    }
}

// MARK: - SwiftUI modal

@MainActor
final class TranscriptionModel: ObservableObject {
    enum Phase { case loading, done, error }
    @Published var phase: Phase = .loading
    @Published var text: String = ""
    @Published var message: String = "Transcribing…"

    private let audioURL: URL
    private let onPersist: (String) -> Void
    let onOpenApp: () -> Void
    let onClose: () -> Void

    init(audioURL: URL, onPersist: @escaping (String) -> Void,
         onOpenApp: @escaping () -> Void, onClose: @escaping () -> Void) {
        self.audioURL = audioURL
        self.onPersist = onPersist
        self.onOpenApp = onOpenApp
        self.onClose = onClose
    }

    func start() {
        guard #available(iOS 17.0, *) else {
            phase = .error
            message = "Transcription needs iOS 17 or later."
            return
        }
        // The model is downloaded by the main app into the App Group; the
        // extension only loads it. If it's missing, ask the user to open Yappy.
        if !YappyTranscriber.shared.isReady {
            phase = .error
            message = "Open Yappy once to download the transcription model, then try again."
            return
        }
        Task {
            let result = await YappyTranscriber.shared.transcribe(path: audioURL.path)
            await MainActor.run {
                if let result, !result.isEmpty {
                    self.text = result
                    self.phase = .done
                    self.onPersist(result)
                } else {
                    self.phase = .error
                    self.message = "Couldn't transcribe that audio."
                }
            }
        }
    }

    func copy() { UIPasteboard.general.string = text }
}

struct TranscriptionView: View {
    @ObservedObject var model: TranscriptionModel
    @State private var copied = false

    var body: some View {
        VStack(spacing: 16) {
            HStack {
                Text("Yappy").font(.headline)
                Spacer()
                Button("Done") { model.onClose() }
            }
            switch model.phase {
            case .loading:
                ProgressView(model.message).frame(maxWidth: .infinity, minHeight: 120)
            case .error:
                VStack(spacing: 12) {
                    Text(model.message).multilineTextAlignment(.center).foregroundColor(.secondary)
                    Button("Open Yappy") { model.onOpenApp() }.buttonStyle(.borderedProminent)
                }.frame(maxWidth: .infinity, minHeight: 120)
            case .done:
                ScrollView {
                    Text(model.text).frame(maxWidth: .infinity, alignment: .leading).textSelection(.enabled)
                }.frame(maxHeight: 280)
                HStack {
                    Button(copied ? "Copied!" : "Copy") {
                        model.copy(); copied = true
                        DispatchQueue.main.asyncAfter(deadline: .now() + 1.2) { copied = false }
                    }.buttonStyle(.bordered)
                    Spacer()
                    Button("Open in Yappy") { model.onOpenApp() }.buttonStyle(.borderedProminent)
                }
            }
        }
        .padding(20)
    }
}
