// UIActivityViewController bridge — presents the iOS share sheet for a
// file URL. Used by the "share rendered .m4b" flow: tap a button, the sheet
// pops up with AirDrop, Apple Books, Messages, Mail, Files, etc. as
// possible destinations.

import UIKit

@_cdecl("yappy_share_file")
public func yappy_share_file(_ pathPtr: UnsafePointer<CChar>?) {
    guard let p = pathPtr else { return }
    let path = String(cString: p)
    let url = URL(fileURLWithPath: path)
    guard FileManager.default.fileExists(atPath: path) else {
        NSLog("[yappy/share] file not found: \(path)")
        return
    }
    DispatchQueue.main.async {
        // Get the top-most view controller to present from. Tauri's iOS app
        // has a single key window with a root view controller.
        guard let root = topViewController() else {
            NSLog("[yappy/share] no root view controller")
            return
        }
        let avc = UIActivityViewController(activityItems: [url], applicationActivities: nil)
        // iPad: anchor the popover to the center of the screen since we
        // don't have a sourceView from Rust.
        if let pop = avc.popoverPresentationController {
            pop.sourceView = root.view
            pop.sourceRect = CGRect(x: root.view.bounds.midX, y: root.view.bounds.midY, width: 0, height: 0)
            pop.permittedArrowDirections = []
        }
        root.present(avc, animated: true)
    }
}

private func topViewController(_ base: UIViewController? = nil) -> UIViewController? {
    // iOS 14-compatible: walk connectedScenes, find any window flagged
    // isKeyWindow. `UIWindowScene.keyWindow` was added in iOS 15 but we
    // target 14 for broader compatibility.
    let base = base ?? UIApplication.shared.connectedScenes
        .compactMap { ($0 as? UIWindowScene)?.windows.first(where: { $0.isKeyWindow })?.rootViewController }
        .first
    if let nav = base as? UINavigationController {
        return topViewController(nav.visibleViewController)
    }
    if let tab = base as? UITabBarController, let selected = tab.selectedViewController {
        return topViewController(selected)
    }
    if let presented = base?.presentedViewController {
        return topViewController(presented)
    }
    return base
}
