// AVAudioSession + silent-loop background-keepalive for Yappy iOS.
//
// iOS suspends UIKit apps within ~30s of going to background unless they are
// "actively producing audio" (UIBackgroundModes = audio). Audiobook renders
// can take HOURS, so we need to keep the app alive for the full render.
//
// Trick: at render start we activate an AVAudioSession (category .playback,
// mixWithOthers) and play a near-silent looped PCM buffer through
// AVAudioEngine. iOS considers us an "audio app" the entire time, so the
// Rust render loop keeps running.
//
// `_cdecl` exposes plain C symbols Rust can `extern "C"` declare without any
// objc2 / framework dance on the Rust side.

import Foundation
import AVFoundation

private actor SilentAudioKeepalive {
    static let shared = SilentAudioKeepalive()

    private var engine: AVAudioEngine?
    private var player: AVAudioPlayerNode?
    private var silentBuffer: AVAudioPCMBuffer?

    func begin() {
        if engine != nil { return } // already running

        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.playback, mode: .default, options: [.mixWithOthers])
            try session.setActive(true)
        } catch {
            NSLog("[yappy/audio] setCategory/setActive failed: \(error)")
            return
        }

        let engine = AVAudioEngine()
        let player = AVAudioPlayerNode()
        engine.attach(player)
        let format = engine.outputNode.outputFormat(forBus: 0)
        engine.connect(player, to: engine.mainMixerNode, format: format)

        // ~1 second of true zeros — looped indefinitely. Inaudible, no
        // battery cost beyond keeping the audio graph open.
        let frameCount = AVAudioFrameCount(format.sampleRate)
        guard let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frameCount) else {
            NSLog("[yappy/audio] couldn't allocate silent PCM buffer")
            return
        }
        buffer.frameLength = frameCount

        do {
            try engine.start()
        } catch {
            NSLog("[yappy/audio] engine.start failed: \(error)")
            return
        }
        player.scheduleBuffer(buffer, at: nil, options: .loops, completionHandler: nil)
        player.play()

        self.engine = engine
        self.player = player
        self.silentBuffer = buffer
        NSLog("[yappy/audio] silent keepalive engaged")
    }

    func end() {
        player?.stop()
        engine?.stop()
        engine = nil
        player = nil
        silentBuffer = nil
        do {
            try AVAudioSession.sharedInstance().setActive(false, options: [.notifyOthersOnDeactivation])
        } catch {
            NSLog("[yappy/audio] setActive(false) failed: \(error)")
        }
        NSLog("[yappy/audio] silent keepalive released")
    }
}

// ─── C-ABI exports — Rust calls these via `extern "C"` ──────────────────

@_cdecl("yappy_background_audio_begin")
public func yappy_background_audio_begin() {
    Task {
        await SilentAudioKeepalive.shared.begin()
    }
}

@_cdecl("yappy_background_audio_end")
public func yappy_background_audio_end() {
    Task {
        await SilentAudioKeepalive.shared.end()
    }
}

// ─── SHARE EXTENSION PAYLOAD DRAINING ────────────────────────────────────
//
// The Share Extension persists incoming URLs/text under
// `shared_payloads` in the App Group's UserDefaults. On launch (or when the
// main app is reopened via the `yappy://` URL scheme) we drain that queue
// and return the most recent payload as a single newline-separated string.
// Rust calls this via `yappy_drain_shared_payload`; ownership of the
// returned C string transfers to Rust (free with `yappy_free_string`).

private let APP_GROUP = "group.com.yappy.app"

@_cdecl("yappy_drain_shared_payload")
public func yappy_drain_shared_payload() -> UnsafeMutablePointer<CChar>? {
    guard let defaults = UserDefaults(suiteName: APP_GROUP) else {
        return nil
    }
    guard let queue = defaults.array(forKey: "shared_payloads") as? [[String: Any]],
          !queue.isEmpty else {
        return nil
    }
    // Drain — clear the queue once read.
    defaults.removeObject(forKey: "shared_payloads")
    // Flatten the queue into a newline-separated string. Each entry is
    // formatted as "url:<absolute>" or "text:<text>" by the Share Extension.
    let joined = queue.compactMap { $0["payload"] as? String }.joined(separator: "\n")
    return strdup(joined)
}

@_cdecl("yappy_free_string")
public func yappy_free_string(_ ptr: UnsafeMutablePointer<CChar>?) {
    free(ptr)
}
