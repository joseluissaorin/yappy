// Lightweight haptic-feedback bridge for iOS.
//
// Browsers (including WKWebView) don't reliably expose Apple's haptic
// generators via the Web Vibration API, so we proxy them through a Tauri
// command that calls into Swift via this @_cdecl bridge.
//
// Three intensities matching the standard Apple HIG categories:
//   - "light" / "medium" / "heavy" = UIImpactFeedbackGenerator.FeedbackStyle
//   - "success" / "warning" / "error" = UINotificationFeedbackGenerator
//   - "selection" = UISelectionFeedbackGenerator
//
// Each call prepares + plays + releases the generator in one shot, which is
// fine for one-off taps. For rapid sequences (e.g. scrubbing) the caller
// would want to hold a generator across multiple invocations — TODO if/when
// we add a scrubbable seek bar.

import UIKit

@_cdecl("yappy_haptic")
public func yappy_haptic(_ kindPtr: UnsafePointer<CChar>?) {
    guard let p = kindPtr else { return }
    let kind = String(cString: p)
    DispatchQueue.main.async {
        switch kind {
        case "light":
            let g = UIImpactFeedbackGenerator(style: .light)
            g.prepare(); g.impactOccurred()
        case "medium":
            let g = UIImpactFeedbackGenerator(style: .medium)
            g.prepare(); g.impactOccurred()
        case "heavy":
            let g = UIImpactFeedbackGenerator(style: .heavy)
            g.prepare(); g.impactOccurred()
        case "selection":
            let g = UISelectionFeedbackGenerator()
            g.prepare(); g.selectionChanged()
        case "success":
            let g = UINotificationFeedbackGenerator()
            g.prepare(); g.notificationOccurred(.success)
        case "warning":
            let g = UINotificationFeedbackGenerator()
            g.prepare(); g.notificationOccurred(.warning)
        case "error":
            let g = UINotificationFeedbackGenerator()
            g.prepare(); g.notificationOccurred(.error)
        default:
            // Unknown kind — silent no-op rather than crash.
            break
        }
    }
}
