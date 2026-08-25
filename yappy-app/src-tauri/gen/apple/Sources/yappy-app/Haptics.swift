// El puente háptico de Yappy, en serio.
//
// WKWebView ignora la Web Vibration API, así que el frontend pasa por un
// comando Tauri que llega aquí por @_cdecl. La versión anterior creaba el
// generador, disparaba y lo SOLTABA en la misma llamada: el Taptic Engine
// descarta peticiones cuyo generador muere antes de reproducirse, y el
// resultado era una app entera con háptica configurada y MUDA.
//
// Doctrina de esta versión:
//   - Generadores ESTÁTICOS de larga vida, uno por estilo.
//   - prepare() al arrancar y RE-prepare tras cada disparo: el motor queda
//     caliente y la latencia baja de ~60 ms a efectivamente cero.
//   - Todo en main, en bloque asíncrono: se puede llamar desde cualquier
//     hilo de Rust sin pensar.
//
// La paleta (alineada con haptic.ts):
//   tick      → UISelectionFeedbackGenerator (posar el dedo, detentes,
//               el paso de frase del cartel)
//   light/soft/medium/rigid/heavy → UIImpactFeedbackGenerator
//   selection → alias histórico de tick
//   success/warning/error → UINotificationFeedbackGenerator

import UIKit

private enum Taptic {
    static let light = UIImpactFeedbackGenerator(style: .light)
    static let soft = UIImpactFeedbackGenerator(style: .soft)
    static let medium = UIImpactFeedbackGenerator(style: .medium)
    static let rigid = UIImpactFeedbackGenerator(style: .rigid)
    static let heavy = UIImpactFeedbackGenerator(style: .heavy)
    static let noti = UINotificationFeedbackGenerator()
    static let sel = UISelectionFeedbackGenerator()

    static let arranque: Void = {
        light.prepare(); soft.prepare(); medium.prepare()
        rigid.prepare(); heavy.prepare(); noti.prepare(); sel.prepare()
    }()
}

@_cdecl("yappy_haptic")
public func yappy_haptic(_ kindPtr: UnsafePointer<CChar>?) {
    guard let p = kindPtr else { return }
    let kind = String(cString: p)
    DispatchQueue.main.async {
        _ = Taptic.arranque
        switch kind {
        case "tick", "selection":
            Taptic.sel.selectionChanged()
            Taptic.sel.prepare()
        case "light":
            Taptic.light.impactOccurred()
            Taptic.light.prepare()
        case "soft":
            Taptic.soft.impactOccurred()
            Taptic.soft.prepare()
        case "medium":
            Taptic.medium.impactOccurred()
            Taptic.medium.prepare()
        case "rigid":
            Taptic.rigid.impactOccurred()
            Taptic.rigid.prepare()
        case "heavy":
            Taptic.heavy.impactOccurred()
            Taptic.heavy.prepare()
        case "success":
            Taptic.noti.notificationOccurred(.success)
            Taptic.noti.prepare()
        case "warning":
            Taptic.noti.notificationOccurred(.warning)
            Taptic.noti.prepare()
        case "error":
            Taptic.noti.notificationOccurred(.error)
            Taptic.noti.prepare()
        default:
            break
        }
    }
}
