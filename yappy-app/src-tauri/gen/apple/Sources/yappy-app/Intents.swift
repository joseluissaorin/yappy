// App Intents: los gestos de Siri, la app Atajos y el botón de acción.
//
// Cada intent ENCOLA una acción en el App Group (el mismo canal que usa la
// Share Extension) y abre la app; el frontend la ejecuta al drenar. Así no
// hay dos caminos que mantener: todo entra por la misma puerta.

import AppIntents
import Foundation

private let GRUPO = "group.com.joseluissaorin.yappy"

private func encolarAccion(_ accion: String) {
    guard let defaults = UserDefaults(suiteName: GRUPO) else { return }
    var cola = defaults.array(forKey: "shared_payloads") as? [[String: Any]] ?? []
    cola.append([
        "payload": "accion:\(accion)",
        "user_title": "",
        "ts": Int(Date().timeIntervalSince1970 * 1000),
    ])
    defaults.set(cola, forKey: "shared_payloads")
}

@available(iOS 16.0, *)
struct LeerPortapapelesIntent: AppIntent {
    static var title: LocalizedStringResource = "Read my clipboard"
    static var description = IntentDescription("Reads the clipboard aloud with Yappy.")
    static var openAppWhenRun = true

    @MainActor
    func perform() async throws -> some IntentResult {
        encolarAccion("read-clipboard")
        return .result()
    }
}

@available(iOS 16.0, *)
struct ReanudarLecturaIntent: AppIntent {
    static var title: LocalizedStringResource = "Resume reading"
    static var description = IntentDescription("Picks up the last listen where you left off.")
    static var openAppWhenRun = true

    @MainActor
    func perform() async throws -> some IntentResult {
        encolarAccion("resume")
        return .result()
    }
}

@available(iOS 16.0, *)
struct EscucharEnlaceIntent: AppIntent {
    static var title: LocalizedStringResource = "Listen to a link"
    static var description = IntentDescription("Queues a link so Yappy reads the article aloud.")
    static var openAppWhenRun = true

    @Parameter(title: "Link")
    var enlace: URL

    @MainActor
    func perform() async throws -> some IntentResult {
        guard let defaults = UserDefaults(suiteName: GRUPO) else { return .result() }
        var cola = defaults.array(forKey: "shared_payloads") as? [[String: Any]] ?? []
        cola.append([
            "payload": "url:\(enlace.absoluteString)",
            "user_title": "",
            "ts": Int(Date().timeIntervalSince1970 * 1000),
        ])
        defaults.set(cola, forKey: "shared_payloads")
        return .result()
    }
}

@available(iOS 16.0, *)
struct AtajosDeYappy: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: LeerPortapapelesIntent(),
            phrases: [
                "Read my clipboard in \(.applicationName)",
                "Lee mi portapapeles con \(.applicationName)",
            ],
            shortTitle: "Read clipboard",
            systemImageName: "doc.on.clipboard"
        )
        AppShortcut(
            intent: ReanudarLecturaIntent(),
            phrases: [
                "Resume reading in \(.applicationName)",
                "Reanuda la lectura en \(.applicationName)",
            ],
            shortTitle: "Resume",
            systemImageName: "play.circle"
        )
        AppShortcut(
            intent: EscucharEnlaceIntent(),
            phrases: [
                "Listen to a link in \(.applicationName)",
                "Escucha un enlace con \(.applicationName)",
            ],
            shortTitle: "Listen to link",
            systemImageName: "link"
        )
    }
}
