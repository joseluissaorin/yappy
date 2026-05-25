// AVAudioPlayer bridge — plays a pre-rendered .m4b / .m4a / .mp3 / .wav file
// in-app, so the Library tab can offer real playback instead of punting to
// Apple Books.
//
// State is a process-wide singleton; Yappy plays one file at a time (the
// TTS synthesis path also runs through one shared output, so there's no
// reason to support concurrent file playback).
//
// Rust polls position via `yappy_audiofile_position` and the existing
// Now Playing infrastructure surfaces the lock-screen controls automatically.

import Foundation
import AVFoundation

private final class AudioFilePlayer: NSObject, AVAudioPlayerDelegate {
    static let shared = AudioFilePlayer()
    private var player: AVAudioPlayer?
    private var currentPath: String?
    private var didFinishCallback: (() -> Void)?

    func play(path: String, startAt: TimeInterval = 0) -> Bool {
        // If we're already playing this file, just resume from where we are.
        if currentPath == path, let p = player {
            p.play()
            return true
        }
        stop()
        let url = URL(fileURLWithPath: path)
        guard FileManager.default.fileExists(atPath: path) else {
            NSLog("[yappy/audiofile] missing file: \(path)")
            return false
        }
        do {
            try AVAudioSession.sharedInstance().setCategory(.playback, mode: .spokenAudio, options: [])
            try AVAudioSession.sharedInstance().setActive(true)
            let p = try AVAudioPlayer(contentsOf: url)
            p.delegate = self
            p.prepareToPlay()
            if startAt > 0 && startAt < p.duration {
                p.currentTime = startAt
            }
            p.play()
            player = p
            currentPath = path
            NSLog("[yappy/audiofile] playing \(url.lastPathComponent), duration=\(p.duration)s")
            return true
        } catch {
            NSLog("[yappy/audiofile] play failed: \(error)")
            return false
        }
    }

    func pause() { player?.pause() }
    func resume() { player?.play() }

    func stop() {
        player?.stop()
        player = nil
        currentPath = nil
    }

    func seek(toSecs secs: TimeInterval) {
        guard let p = player else { return }
        p.currentTime = max(0, min(p.duration, secs))
    }

    func position() -> TimeInterval { player?.currentTime ?? 0 }
    func duration() -> TimeInterval { player?.duration ?? 0 }
    func isPlaying() -> Bool { player?.isPlaying ?? false }
    func current() -> String? { currentPath }

    func audioPlayerDidFinishPlaying(_ p: AVAudioPlayer, successfully _: Bool) {
        currentPath = nil
        player = nil
    }
}

// ─── C ABI ──────────────────────────────────────────────────────────────

@_cdecl("yappy_audiofile_play")
public func yappy_audiofile_play(_ pathPtr: UnsafePointer<CChar>?, _ startAt: Double) -> Bool {
    guard let p = pathPtr else { return false }
    let path = String(cString: p)
    return AudioFilePlayer.shared.play(path: path, startAt: startAt)
}

@_cdecl("yappy_audiofile_pause")
public func yappy_audiofile_pause() { AudioFilePlayer.shared.pause() }

@_cdecl("yappy_audiofile_resume")
public func yappy_audiofile_resume() { AudioFilePlayer.shared.resume() }

@_cdecl("yappy_audiofile_stop")
public func yappy_audiofile_stop() { AudioFilePlayer.shared.stop() }

@_cdecl("yappy_audiofile_seek")
public func yappy_audiofile_seek(_ secs: Double) {
    AudioFilePlayer.shared.seek(toSecs: secs)
}

@_cdecl("yappy_audiofile_position")
public func yappy_audiofile_position() -> Double { AudioFilePlayer.shared.position() }

@_cdecl("yappy_audiofile_duration")
public func yappy_audiofile_duration() -> Double { AudioFilePlayer.shared.duration() }

@_cdecl("yappy_audiofile_is_playing")
public func yappy_audiofile_is_playing() -> Bool { AudioFilePlayer.shared.isPlaying() }

@_cdecl("yappy_audiofile_current_path")
public func yappy_audiofile_current_path() -> UnsafeMutablePointer<CChar>? {
    guard let p = AudioFilePlayer.shared.current() else { return nil }
    return strdup(p)
}
