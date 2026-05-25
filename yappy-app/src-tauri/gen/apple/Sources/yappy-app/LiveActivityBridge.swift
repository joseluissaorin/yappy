// ActivityKit ↔ Rust bridge.
//
// Rust calls these `@_cdecl` C ABI functions when an audiobook render starts /
// makes progress / finishes. We start a single ActivityKit activity for the
// duration of the render and update its ContentState as progress events
// arrive. The Widget Extension (YappyWidgets) handles the Lock Screen and
// Dynamic Island UI.
//
// `currentActivity` is a process-wide singleton — only one render activity
// at a time is supported (audiobook renders are CPU-bound, no reason to
// parallelize them on a phone).

import Foundation
import ActivityKit

@available(iOS 16.2, *)
private actor LiveActivityState {
    static let shared = LiveActivityState()
    private var current: Activity<RenderActivityAttributes>?

    func start(title: String, total: Int) {
        guard current == nil else { return }
        guard ActivityAuthorizationInfo().areActivitiesEnabled else {
            NSLog("[yappy/activity] Live Activities disabled by user; skipping")
            return
        }
        do {
            let attrs = RenderActivityAttributes()
            let state = RenderActivityAttributes.State(
                title: title, done: 0, total: total, stage: "synth")
            let activity = try Activity.request(
                attributes: attrs,
                content: .init(state: state, staleDate: nil),
                pushType: nil
            )
            self.current = activity
            NSLog("[yappy/activity] started \(activity.id) title=\(title) total=\(total)")
        } catch {
            NSLog("[yappy/activity] start failed: \(error)")
        }
    }

    func update(done: Int, total: Int, stage: String, title: String?) async {
        guard let activity = current else { return }
        let state = RenderActivityAttributes.State(
            title: title ?? activity.content.state.title,
            done: done, total: total, stage: stage)
        await activity.update(.init(state: state, staleDate: nil))
    }

    func end(title: String) async {
        guard let activity = current else { return }
        let finalState = RenderActivityAttributes.State(
            title: title, done: activity.content.state.total,
            total: activity.content.state.total, stage: "done")
        await activity.end(.init(state: finalState, staleDate: nil), dismissalPolicy: .default)
        current = nil
        NSLog("[yappy/activity] ended")
    }
}

// ─── C-ABI exports — Rust calls these via `extern "C"` ───────────────────

// Converts a Rust UTF-8 C string (or NULL) into a Swift String.
private func swiftStr(_ ptr: UnsafePointer<CChar>?) -> String {
    guard let p = ptr else { return "" }
    return String(cString: p)
}

@_cdecl("yappy_activity_start")
public func yappy_activity_start(_ titlePtr: UnsafePointer<CChar>?, _ total: Int32) {
    let title = swiftStr(titlePtr)
    let totalI = Int(total)
    if #available(iOS 16.2, *) {
        Task { await LiveActivityState.shared.start(title: title, total: totalI) }
    }
}

@_cdecl("yappy_activity_update")
public func yappy_activity_update(_ done: Int32, _ total: Int32, _ stagePtr: UnsafePointer<CChar>?, _ titlePtr: UnsafePointer<CChar>?) {
    let stage = swiftStr(stagePtr)
    let title = titlePtr != nil ? swiftStr(titlePtr) : nil
    if #available(iOS 16.2, *) {
        Task { await LiveActivityState.shared.update(done: Int(done), total: Int(total), stage: stage, title: title) }
    }
}

@_cdecl("yappy_activity_end")
public func yappy_activity_end(_ titlePtr: UnsafePointer<CChar>?) {
    let title = swiftStr(titlePtr)
    if #available(iOS 16.2, *) {
        Task { await LiveActivityState.shared.end(title: title) }
    }
}
