// CoreSpotlight bridge — indexes the saved audiobook library so iOS users
// can find their renders via Spotlight (swipe down on home screen, type a
// keyword). Each item becomes a CSSearchableItem with title + duration +
// chapter count + a yappy://library/<path-encoded> tap-to-open URL.

import Foundation
import CoreSpotlight
import UniformTypeIdentifiers

private final class SpotlightIndexer {
    static let shared = SpotlightIndexer()
    private let domain = "com.joseluissaorin.yappy.library"

    /// Replace the entire library domain with the supplied items. Older
    /// entries that aren't in `items` get dropped automatically.
    func replaceAll(_ items: [(path: String, name: String, durationSecs: Double, chapters: Int)]) {
        let csItems: [CSSearchableItem] = items.map { item in
            // "public.audiobook" is the system UTI for audiobook content.
            // UTType.audiobook (Swift typed) was only added in iOS 17; the
            // raw string works back to iOS 9.
            let attrs = CSSearchableItemAttributeSet(itemContentType: "public.audiobook")
            attrs.title = item.name
            attrs.displayName = item.name
            attrs.contentDescription = chapterText(for: item)
            attrs.duration = NSNumber(value: item.durationSecs)
            attrs.keywords = ["audiobook", "yappy", "tts", "yapped"]
            attrs.thumbnailData = nil  // cover art (TODO Phase 3 of library) lands here.
            // URL the OS launches the app with when the user taps this
            // Spotlight result. Existing yappy:// handling in the main app
            // brings the foreground; v0.2 will route to the library row +
            // start playback.
            let encodedPath = item.path
                .addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? ""
            attrs.contentURL = URL(string: "yappy://library?path=\(encodedPath)")
            return CSSearchableItem(
                uniqueIdentifier: item.path,
                domainIdentifier: domain,
                attributeSet: attrs
            )
        }
        let index = CSSearchableIndex.default()
        index.deleteSearchableItems(withDomainIdentifiers: [domain]) { err in
            if let err = err {
                NSLog("[yappy/spotlight] delete-domain failed: \(err)")
            }
            index.indexSearchableItems(csItems) { err in
                if let err = err {
                    NSLog("[yappy/spotlight] index failed: \(err)")
                } else {
                    NSLog("[yappy/spotlight] indexed \(csItems.count) items")
                }
            }
        }
    }

    private func chapterText(for item: (path: String, name: String, durationSecs: Double, chapters: Int)) -> String {
        let mins = Int(item.durationSecs / 60)
        if item.chapters > 0 {
            return "\(mins) min · \(item.chapters) chapters"
        }
        return "\(mins) min audiobook"
    }
}

// ─── C ABI ──────────────────────────────────────────────────────────────
//
// Rust passes a flat JSON-ish payload as a single string: each line is
//   "<path>\t<name>\t<duration_secs>\t<chapter_count>"
// We split + index. Single-string keeps the FFI dead simple — no struct
// marshalling across the Rust→Swift boundary.

@_cdecl("yappy_spotlight_replace_all")
public func yappy_spotlight_replace_all(_ payloadPtr: UnsafePointer<CChar>?) {
    guard let p = payloadPtr else { return }
    let payload = String(cString: p)
    let items: [(String, String, Double, Int)] = payload
        .split(separator: "\n", omittingEmptySubsequences: true)
        .compactMap { line -> (String, String, Double, Int)? in
            let parts = line.split(separator: "\t", maxSplits: 3, omittingEmptySubsequences: false)
            guard parts.count == 4 else { return nil }
            let path = String(parts[0])
            let name = String(parts[1])
            let dur = Double(parts[2]) ?? 0
            let ch = Int(parts[3]) ?? 0
            return (path, name, dur, ch)
        }
    SpotlightIndexer.shared.replaceAll(items)
}
