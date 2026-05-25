// Local user notifications.
//
// Two use cases for v0.1:
//   1. Fire when a long m4b render completes — the user almost certainly
//      switched apps or locked the phone during a multi-hour render, so a
//      local notification gets their attention back.
//   2. Fire when the sleep timer expires (TODO — currently the timer just
//      stops playback silently).
//
// We request notification permission lazily on first call. The system
// caches the answer, so subsequent calls are no-ops if already granted/denied.

import Foundation
import UserNotifications

private var permissionAsked = false

private func ensurePermission(_ then: @escaping (Bool) -> Void) {
    let center = UNUserNotificationCenter.current()
    center.getNotificationSettings { settings in
        switch settings.authorizationStatus {
        case .authorized, .provisional:
            then(true)
        case .denied:
            then(false)
        case .notDetermined:
            if permissionAsked { then(false); return }
            permissionAsked = true
            center.requestAuthorization(options: [.alert, .sound, .badge]) { granted, _ in
                then(granted)
            }
        @unknown default:
            then(false)
        }
    }
}

/// Fire an immediate local notification. Rust calls this when an m4b
/// render finishes. Title and body are UTF-8 C strings. `identifier`
/// dedupes — passing the same identifier twice replaces the earlier one.
@_cdecl("yappy_notify")
public func yappy_notify(
    _ identifierPtr: UnsafePointer<CChar>?,
    _ titlePtr: UnsafePointer<CChar>?,
    _ bodyPtr: UnsafePointer<CChar>?
) {
    let identifier = identifierPtr.map { String(cString: $0) } ?? UUID().uuidString
    let title = titlePtr.map { String(cString: $0) } ?? "Yappy"
    let body  = bodyPtr.map { String(cString: $0) } ?? ""

    ensurePermission { granted in
        guard granted else {
            NSLog("[yappy/notify] permission denied — skipping notification \(identifier)")
            return
        }
        let content = UNMutableNotificationContent()
        content.title = title
        content.body = body
        content.sound = .default

        let trigger = UNTimeIntervalNotificationTrigger(timeInterval: 0.1, repeats: false)
        let request = UNNotificationRequest(identifier: identifier, content: content, trigger: trigger)
        UNUserNotificationCenter.current().add(request) { err in
            if let err = err {
                NSLog("[yappy/notify] add failed: \(err)")
            }
        }
    }
}
