// Home-screen widget — appears in the user's iOS widget gallery alongside
// Weather / Calendar / etc. Bundled in the same Widget Extension target as
// the Live Activity (YappyWidgets.appex), so no new target needed.
//
// Tap → opens yappy://action/read-clipboard → the main app's launch logic
// detects fresh pasteboard content and offers to read it.

import WidgetKit
import SwiftUI

// ─── Timeline provider ─────────────────────────────────────────────────
// Reads the last-played title from the App Group's UserDefaults. The
// main app writes to that whenever a playback session starts (see
// AppState.last_played_title in v0.2). For now the widget falls back to
// generic copy if no title is present.

struct YappyHomeEntry: TimelineEntry {
    let date: Date
    let lastTitle: String?
}

struct YappyHomeProvider: TimelineProvider {
    private static let APP_GROUP = "group.com.joseluissaorin.yappy"

    func placeholder(in context: Context) -> YappyHomeEntry {
        YappyHomeEntry(date: Date(), lastTitle: nil)
    }

    func getSnapshot(in context: Context, completion: @escaping (YappyHomeEntry) -> Void) {
        completion(read())
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<YappyHomeEntry>) -> Void) {
        let next = Calendar.current.date(byAdding: .minute, value: 30, to: Date())!
        completion(Timeline(entries: [read()], policy: .after(next)))
    }

    private func read() -> YappyHomeEntry {
        let defaults = UserDefaults(suiteName: Self.APP_GROUP)
        let title = defaults?.string(forKey: "last_played_title")
        return YappyHomeEntry(date: Date(), lastTitle: title)
    }
}

// ─── Views ─────────────────────────────────────────────────────────────

struct YappyHomeWidgetEntryView: View {
    @Environment(\.widgetFamily) var family
    let entry: YappyHomeEntry

    var body: some View {
        Group {
            switch family {
            case .systemMedium:
                mediumLayout
            default:
                smallLayout
            }
        }
        // Tap → main app launches with this URL → existing yappy:// handling
        // brings the clipboard banner up.
        .widgetURL(URL(string: "yappy://action/read-clipboard"))
    }

    private var smallLayout: some View {
        VStack(alignment: .leading, spacing: 6) {
            Image(systemName: "waveform.circle.fill")
                .font(.system(size: 28))
                .foregroundColor(.pink)
            Spacer(minLength: 4)
            Text("yappy")
                .font(.system(size: 18, weight: .heavy, design: .rounded))
                .foregroundColor(.pink)
            Text(entry.lastTitle ?? "read anything aloud")
                .font(.caption)
                .foregroundColor(.secondary)
                .lineLimit(2)
        }
        .padding(14)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    private var mediumLayout: some View {
        HStack(spacing: 14) {
            Image(systemName: "waveform.circle.fill")
                .font(.system(size: 40))
                .foregroundColor(.pink)
            VStack(alignment: .leading, spacing: 6) {
                Text("yappy")
                    .font(.system(size: 20, weight: .heavy, design: .rounded))
                    .foregroundColor(.pink)
                Text(entry.lastTitle ?? "tap to read your clipboard")
                    .font(.callout)
                    .lineLimit(2)
                Spacer(minLength: 0)
                HStack(spacing: 6) {
                    Image(systemName: "play.fill")
                        .font(.system(size: 11, weight: .bold))
                    Text("read clipboard")
                        .font(.caption.weight(.semibold))
                }
                .foregroundColor(.white)
                .padding(.horizontal, 12)
                .padding(.vertical, 6)
                .background(Color.pink)
                .clipShape(Capsule())
            }
            Spacer()
        }
        .padding(16)
    }
}

@available(iOS 16.0, *)
struct YappyHomeWidget: Widget {
    let kind = "YappyHomeWidget"

    var body: some WidgetConfiguration {
        StaticConfiguration(kind: kind, provider: YappyHomeProvider()) { entry in
            YappyHomeWidgetEntryView(entry: entry)
        }
        .configurationDisplayName("Yappy")
        .description("Tap to read your clipboard aloud.")
        .supportedFamilies([.systemSmall, .systemMedium])
    }
}
