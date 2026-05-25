// Share Extension — receives a URL or text from any iOS app's Share Sheet and
// queues it into the App Group container shared with the main Yappy app. The
// main app picks it up on next launch via mobile::pickup_shared_payload and
// reads it aloud.
//
// We deliberately ship a minimal UI: dismiss instantly, no confirmation
// dialog — the user already pressed Share → Yappy, no need to ask twice.

import UIKit
import Social
import UniformTypeIdentifiers

@objc(ShareViewController)
class ShareViewController: SLComposeServiceViewController {

    // App Group container shared with the main app. Make sure
    // `group.com.yappy.app` is registered in Apple Developer portal under
    // Identifiers → App Groups and enabled on both this extension's App ID
    // (com.yappy.app.ShareExtension) and the main app's (com.yappy.app).
    private static let APP_GROUP = "group.com.yappy.app"

    override func isContentValid() -> Bool { true }

    override func didSelectPost() {
        guard let items = (extensionContext?.inputItems as? [NSExtensionItem]) else {
            close()
            return
        }

        let group = DispatchGroup()
        var captured: [String] = []
        let userTitle = self.contentText ?? ""

        for item in items {
            for provider in (item.attachments ?? []) {
                if provider.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
                    group.enter()
                    provider.loadItem(forTypeIdentifier: UTType.url.identifier, options: nil) { (data, _) in
                        if let url = data as? URL {
                            captured.append("url:\(url.absoluteString)")
                        }
                        group.leave()
                    }
                } else if provider.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
                    group.enter()
                    provider.loadItem(forTypeIdentifier: UTType.plainText.identifier, options: nil) { (data, _) in
                        if let text = data as? String {
                            captured.append("text:\(text)")
                        }
                        group.leave()
                    }
                }
            }
        }

        group.notify(queue: .main) {
            self.persistPayload(items: captured, userTitle: userTitle)
            // Open the main app via custom URL scheme so iOS surfaces it now,
            // rather than waiting for the next manual launch.
            self.openMainApp()
            self.close()
        }
    }

    override func configurationItems() -> [Any]! { [] }

    private func persistPayload(items: [String], userTitle: String) {
        guard let defaults = UserDefaults(suiteName: ShareViewController.APP_GROUP) else {
            NSLog("[yappy/share] App Group UserDefaults unavailable — check entitlements")
            return
        }
        // Existing queue + append.
        var queue = defaults.array(forKey: "shared_payloads") as? [[String: Any]] ?? []
        for item in items {
            queue.append([
                "payload": item,                                  // "url:..." or "text:..."
                "user_title": userTitle,
                "ts": Int(Date().timeIntervalSince1970 * 1000),
            ])
        }
        defaults.set(queue, forKey: "shared_payloads")
        NSLog("[yappy/share] persisted \(items.count) item(s); queue now \(queue.count)")
    }

    /// Open the main Yappy app via the registered custom URL scheme.
    /// The main app's Info.plist must declare `yappy://` under CFBundleURLTypes
    /// for this to work; otherwise the share completes silently and the user
    /// has to relaunch the app manually.
    private func openMainApp() {
        guard let url = URL(string: "yappy://shared") else { return }
        var responder: UIResponder? = self
        while responder != nil {
            if let app = responder as? UIApplication {
                app.perform(#selector(UIApplication.open(_:options:completionHandler:)), with: url)
                return
            }
            responder = responder?.next
        }
    }

    private func close() {
        extensionContext?.completeRequest(returningItems: [], completionHandler: nil)
    }
}
