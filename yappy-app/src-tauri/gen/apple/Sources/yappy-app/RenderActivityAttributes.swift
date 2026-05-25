// Shared ActivityKit attributes for the audiobook-render Live Activity.
//
// Imported by BOTH the main app target (which starts/updates/ends the
// activity from Rust via LiveActivityBridge.swift) AND the YappyWidgets
// extension target (which renders the Lock Screen + Dynamic Island UI).
// The activity content is updated whenever Rust emits a render-progress
// event from `render_audiobook_cmd`.

import Foundation
import ActivityKit

@available(iOS 16.1, *)
public struct RenderActivityAttributes: ActivityAttributes {
    public typealias ContentState = State

    // ContentState: the bits that change over the lifetime of the activity.
    // Apple recommends keeping this small (≤4 KB) — strings + numbers only.
    public struct State: Codable, Hashable {
        /// Document title being rendered (e.g. "The Brothers Karamazov.epub").
        public var title: String
        /// Number of paragraphs (or AAC frames) finished so far.
        public var done: Int
        /// Total work units to do. 0 = indeterminate.
        public var total: Int
        /// What we're currently doing — "synth" or "writing" (the same
        /// stages `render_audiobook_cmd` emits via Tauri events).
        public var stage: String

        public init(title: String, done: Int, total: Int, stage: String) {
            self.title = title
            self.done = done
            self.total = total
            self.stage = stage
        }

        /// 0.0…1.0 progress. Returns 0 when `total == 0`.
        public var fraction: Double {
            guard total > 0 else { return 0 }
            return min(1.0, Double(done) / Double(total))
        }
    }

    /// Fixed attributes set at activity start, immutable thereafter.
    /// Currently empty — title is in State so the user can rename it
    /// mid-render if needed.
}
