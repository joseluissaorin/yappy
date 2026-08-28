// Share Extension: la puerta de entrada de Yappy.
//
// Todo lo compartido (una URL, un texto, una nota de voz, un PDF, un EPUB,
// un Word) se ENCOLA en el App Group y se reabre la app, que lo mete en la
// cola con estado visible y hace la extracción con su presupuesto de
// memoria completo. Aquí dentro no se procesa nada: el límite de ~120 MB
// de las extensiones convierte cualquier modelo o parser grande en un
// jetsam seguro.

import UIKit
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

    /// UTIs de documento que aceptamos como FICHERO (se copian al App Group
    /// tal cual, sin cargarlos en memoria: el límite de ~120 MB de las
    /// extensiones no perdona). El DOCX no tiene constante estática.
    private static let documentUTIs: [String] = [
        UTType.pdf.identifier,
        UTType.epub.identifier,
        "org.openxmlformats.wordprocessingml.document",
        "com.microsoft.word.doc",
        "org.oasis-open.opendocument.text",
        UTType.rtf.identifier,
    ]

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
        // Documentos (PDF, EPUB, Word…) antes que url/text, porque Safari
        // adjunta un PDF Y su URL a la vez y queremos el fichero.
        for item in items {
            for provider in (item.attachments ?? []) {
                for uti in Self.documentUTIs where provider.hasItemConformingToTypeIdentifier(uti) {
                    loadFile(provider, uti: uti)
                    return
                }
            }
        }
        // LA VÍA CON SESIÓN: si el share viene de Safari, el preprocesado
        // JS (YappyPreprocess.js) entrega el DOM que el usuario VEÍA, con
        // su suscripción y sin muro. Llega como property-list.
        for item in items {
            for provider in (item.attachments ?? []) {
                if provider.hasItemConformingToTypeIdentifier("com.apple.property-list") {
                    loadWebPage(provider, fallback: items)
                    return
                }
            }
        }
        // Sin audio ni ficheros: el camino clásico de texto/URL.
        handleTextOrUrl(items)
    }

    // MARK: - Página web viva (preprocesado JS de Safari)

    private func loadWebPage(_ provider: NSItemProvider, fallback items: [NSExtensionItem]) {
        provider.loadItem(forTypeIdentifier: "com.apple.property-list", options: nil) { [weak self] data, _ in
            guard let self else { return }
            var payload: String?
            if let dict = data as? NSDictionary,
               let results = dict[NSExtensionJavaScriptPreprocessingResultsKey] as? NSDictionary {
                let url = (results["url"] as? String) ?? ""
                let html = (results["html"] as? String) ?? ""
                let titulo = ((results["titulo"] as? String) ?? "")
                    .replacingOccurrences(of: "-->", with: " ")
                if !html.isEmpty, !url.isEmpty,
                   let container = FileManager.default
                       .containerURL(forSecurityApplicationGroupIdentifier: APP_GROUP) {
                    let dir = container.appendingPathComponent("shared-files", isDirectory: true)
                    try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
                    let dest = dir.appendingPathComponent("pagina-\(UUID().uuidString.prefix(8)).html")
                    // La URL y el título van en la cabecera del propio
                    // fichero: el payload queda en una sola línea.
                    let cuerpo = "<!-- yappy-url: \(url.replacingOccurrences(of: "-->", with: "")) -->\n"
                        + "<!-- yappy-titulo: \(titulo) -->\n" + html
                    if (try? cuerpo.write(to: dest, atomically: true, encoding: .utf8)) != nil {
                        payload = "web:\(dest.path)"
                        NSLog("[yappy/share] página viva capturada (\(html.count) chars): \(url.prefix(60))")
                    }
                }
            }
            DispatchQueue.main.async {
                if let payload {
                    self.persistPayload(payload)
                    self.openMainAppAndClose()
                } else {
                    // Sin resultados del JS: el camino clásico de URL.
                    NSLog("[yappy/share] property-list sin resultados JS; voy por la URL")
                    self.handleTextOrUrl(items)
                }
            }
        }
    }

    // MARK: - Documento → copiar al App Group y encolar

    private func loadFile(_ provider: NSItemProvider, uti: String) {
        // loadFileRepresentation entrega un temporal SIN cargarlo en RAM; hay
        // que copiarlo dentro del callback (el temporal muere al volver).
        provider.loadFileRepresentation(forTypeIdentifier: uti) { [weak self] url, error in
            guard let self else { return }
            var sharedURL: URL?
            if let url {
                sharedURL = self.copyToSharedFiles(url)
            } else {
                NSLog("[yappy/share] loadFileRepresentation(\(uti)) failed: \(String(describing: error))")
            }
            DispatchQueue.main.async {
                if let sharedURL {
                    NSLog("[yappy/share] queued file:\(sharedURL.lastPathComponent)")
                    self.persistPayload("file:\(sharedURL.path)")
                    self.openMainAppAndClose()
                } else {
                    self.close()
                }
            }
        }
    }

    private func copyToSharedFiles(_ url: URL) -> URL? {
        guard let container = FileManager.default
            .containerURL(forSecurityApplicationGroupIdentifier: APP_GROUP) else { return nil }
        let dir = container.appendingPathComponent("shared-files", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let ext = url.pathExtension.isEmpty ? "bin" : url.pathExtension
        let base = url.deletingPathExtension().lastPathComponent
            .replacingOccurrences(of: "/", with: "-")
            .prefix(60)
        let dest = dir.appendingPathComponent("\(base)-\(UUID().uuidString.prefix(6)).\(ext)")
        do {
            try FileManager.default.copyItem(at: url, to: dest)
            return dest
        } catch {
            NSLog("[yappy/share] copy failed: \(error)")
            return nil
        }
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
    /// iOS no da API oficial para esto desde una extensión de compartir, así
    /// que se intenta una CASCADA de técnicas conocidas, de la más limpia a
    /// la más terca, con log por etapa para diagnosticar en el aparato:
    ///   1. cadena de responders + selector de UN argumento `openURL:`
    ///      (funcionaba hasta iOS 17; en versiones nuevas la cadena a veces
    ///      ya no llega a nadie que responda),
    ///   2. cadena de responders + `openURL:options:completionHandler:`
    ///      llamado por IMP (perform solo pasa un argumento; con el puntero
    ///      C se pasan los tres),
    ///   3. `UIApplication` por reflexión (`sharedApplication` vía KVC: la
    ///      API está vetada en tiempo de compilación para extensiones, pero
    ///      el objeto existe en tiempo de ejecución),
    ///   4. `NSExtensionContext.open` (documentado solo para widgets, pero
    ///      gratis intentarlo).
    /// Después se completa la petición con un respiro para que el salto
    /// tenga tiempo de despegar.
    private func openMainAppAndClose() {
        guard let url = URL(string: "yappy://shared") else { close(); return }
        NSLog("[yappy/share] abriendo la app contenedora…")
        let sel1 = NSSelectorFromString("openURL:")
        let sel3 = NSSelectorFromString("openURL:options:completionHandler:")
        typealias AbrirTres = @convention(c) (NSObject, Selector, NSURL, NSDictionary, Any?) -> Void
        var opened = false

        // SOLO sobre la instancia real de UIApplication: otros responders
        // «responden» al selector pero LANZAN NSException al invocarlo (el
        // veto de extensiones), y una excepción aquí mata el proceso sin
        // completar la petición: la hoja se queda gris para siempre.
        if let appClass = NSClassFromString("UIApplication") {
            var responder: UIResponder? = self
            while let r = responder {
                if r.isKind(of: appClass), let obj = r as? NSObject {
                    // El de TRES argumentos primero: en iOS moderno el
                    // «openURL:» clásico responde pero es un no-op para
                    // abrir otra app; el moderno sí despega.
                    if r.responds(to: sel3) {
                        let imp = obj.method(for: sel3)
                        let fn = unsafeBitCast(imp, to: AbrirTres.self)
                        fn(obj, sel3, url as NSURL, [:] as NSDictionary, nil)
                        opened = true
                        NSLog("[yappy/share] salto vía openURL:options: (IMP) en \(type(of: r))")
                    } else if r.responds(to: sel1) {
                        r.perform(sel1, with: url)
                        opened = true
                        NSLog("[yappy/share] salto vía openURL: en \(type(of: r))")
                    }
                    break
                }
                responder = r.next
            }
        }

        if !opened {
            NSLog("[yappy/share] sin UIApplication en la cadena; probando extensionContext.open")
            extensionContext?.open(url, completionHandler: { ok in
                NSLog("[yappy/share] extensionContext.open → \(ok)")
            })
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { self.close() }
    }

    private func close() {
        // Idempotent — the watchdog and the normal completion can both fire.
        guard !finished else { return }
        finished = true
        extensionContext?.completeRequest(returningItems: [], completionHandler: nil)
    }
}
