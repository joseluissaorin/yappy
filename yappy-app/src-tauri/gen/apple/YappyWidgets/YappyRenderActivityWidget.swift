// Live Activity widget for an in-progress audiobook render.
//
// Three places this UI shows up:
//   1. Lock Screen — full card with title, progress bar, percent complete.
//   2. Dynamic Island, compact (one-line near the camera cutout) — just a
//      mascot + percent.
//   3. Dynamic Island, expanded (when long-pressed) — title + progress bar.
//   4. Dynamic Island, minimal (when multiple activities present) — pill.

import ActivityKit
import WidgetKit
import SwiftUI

@available(iOS 16.2, *)
struct YappyRenderActivityWidget: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: RenderActivityAttributes.self) { context in
            // ─── Lock Screen / banner presentation ───────────────────────
            HStack(spacing: 14) {
                Image(systemName: "waveform.circle.fill")
                    .font(.system(size: 36))
                    .foregroundColor(.pink)
                VStack(alignment: .leading, spacing: 4) {
                    Text(context.state.title)
                        .font(.headline)
                        .lineLimit(1)
                    HStack {
                        ProgressView(value: context.state.fraction)
                            .progressViewStyle(.linear)
                            .tint(.pink)
                        Text(percentText(context.state.fraction))
                            .font(.caption.monospacedDigit())
                            .foregroundColor(.secondary)
                    }
                    Text(stageLabel(context.state.stage))
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }
                Spacer()
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .activityBackgroundTint(Color.black.opacity(0.85))
            .activitySystemActionForegroundColor(.pink)

        } dynamicIsland: { context in
            // ─── Dynamic Island ─────────────────────────────────────────
            DynamicIsland {
                // EXPANDED — shown when the user long-presses the island.
                DynamicIslandExpandedRegion(.leading) {
                    Image(systemName: "waveform.circle.fill")
                        .font(.title2)
                        .foregroundColor(.pink)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    Text(percentText(context.state.fraction))
                        .font(.subheadline.monospacedDigit())
                        .foregroundColor(.pink)
                }
                DynamicIslandExpandedRegion(.center) {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(context.state.title)
                            .font(.subheadline.bold())
                            .lineLimit(1)
                        Text(stageLabel(context.state.stage))
                            .font(.caption2)
                            .foregroundColor(.secondary)
                    }
                }
                DynamicIslandExpandedRegion(.bottom) {
                    ProgressView(value: context.state.fraction)
                        .progressViewStyle(.linear)
                        .tint(.pink)
                }
            } compactLeading: {
                // COMPACT — left of the cutout. Mascot only.
                Image(systemName: "waveform")
                    .foregroundColor(.pink)
            } compactTrailing: {
                // COMPACT — right of the cutout. Tiny percent.
                Text(percentText(context.state.fraction))
                    .font(.caption2.monospacedDigit())
                    .foregroundColor(.pink)
            } minimal: {
                // MINIMAL — when stacked with other activities.
                Image(systemName: "waveform")
                    .foregroundColor(.pink)
            }
            .widgetURL(URL(string: "yappy://render"))
            .keylineTint(.pink)
        }
    }
}

private func percentText(_ f: Double) -> String {
    let pct = Int((f * 100).rounded())
    return "\(pct)%"
}

private func stageLabel(_ stage: String) -> String {
    switch stage {
    case "synth":    return "rendering audio…"
    case "writing":  return "writing .m4b…"
    case "done":     return "done"
    default:         return stage
    }
}
