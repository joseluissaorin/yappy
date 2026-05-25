// MPNowPlayingInfoCenter + MPRemoteCommandCenter bridge.
//
// Surfaces Yappy's current playback in the iOS system "Now Playing" UI:
//   - Lock screen (full media controls + artwork)
//   - Control Center
//   - AirPods / CarPlay / Apple Watch / external Bluetooth remotes
//
// Rust updates the playing metadata via `yappy_now_playing_set` whenever the
// playback state changes (play/pause/seek/title-change). Rust registers
// callback function pointers via `yappy_register_*` at startup; iOS calls
// them when the user taps lock-screen controls.

import Foundation
import MediaPlayer
import AVFoundation

// ─── C-ABI callback type for Rust handlers ───────────────────────────────
public typealias YappyVoidCallback = @convention(c) () -> Void
public typealias YappySeekCallback = @convention(c) (Double) -> Void

private var playHandler: YappyVoidCallback?
private var pauseHandler: YappyVoidCallback?
private var togglePlayPauseHandler: YappyVoidCallback?
private var skipForwardHandler: YappyVoidCallback?
private var skipBackwardHandler: YappyVoidCallback?
private var seekHandler: YappySeekCallback?

private var remoteCommandsConfigured = false

/// Wire up MPRemoteCommandCenter handlers once. Safe to call multiple times.
private func configureRemoteCommands() {
    guard !remoteCommandsConfigured else { return }
    remoteCommandsConfigured = true
    let center = MPRemoteCommandCenter.shared()

    center.playCommand.addTarget { _ in
        playHandler?()
        return .success
    }
    center.pauseCommand.addTarget { _ in
        pauseHandler?()
        return .success
    }
    center.togglePlayPauseCommand.addTarget { _ in
        togglePlayPauseHandler?()
        return .success
    }
    // Skip forward/back: jump 15 s, the audiobook-app standard.
    center.skipForwardCommand.preferredIntervals = [15]
    center.skipForwardCommand.addTarget { _ in
        skipForwardHandler?()
        return .success
    }
    center.skipBackwardCommand.preferredIntervals = [15]
    center.skipBackwardCommand.addTarget { _ in
        skipBackwardHandler?()
        return .success
    }
    // Scrubbing (drag the lock-screen progress bar).
    center.changePlaybackPositionCommand.addTarget { event in
        guard let e = event as? MPChangePlaybackPositionCommandEvent else {
            return .commandFailed
        }
        seekHandler?(e.positionTime)
        return .success
    }
    // We don't use next/previous track — disable so they don't show up.
    center.nextTrackCommand.isEnabled = false
    center.previousTrackCommand.isEnabled = false

    NSLog("[yappy/nowplaying] remote command center wired")
}

// ─── PUBLIC FFI ──────────────────────────────────────────────────────────

@_cdecl("yappy_register_play_handler")
public func yappy_register_play_handler(_ cb: YappyVoidCallback?) {
    playHandler = cb
    configureRemoteCommands()
}
@_cdecl("yappy_register_pause_handler")
public func yappy_register_pause_handler(_ cb: YappyVoidCallback?) {
    pauseHandler = cb
    configureRemoteCommands()
}
@_cdecl("yappy_register_toggle_play_pause_handler")
public func yappy_register_toggle_play_pause_handler(_ cb: YappyVoidCallback?) {
    togglePlayPauseHandler = cb
    configureRemoteCommands()
}
@_cdecl("yappy_register_skip_forward_handler")
public func yappy_register_skip_forward_handler(_ cb: YappyVoidCallback?) {
    skipForwardHandler = cb
    configureRemoteCommands()
}
@_cdecl("yappy_register_skip_backward_handler")
public func yappy_register_skip_backward_handler(_ cb: YappyVoidCallback?) {
    skipBackwardHandler = cb
    configureRemoteCommands()
}
@_cdecl("yappy_register_seek_handler")
public func yappy_register_seek_handler(_ cb: YappySeekCallback?) {
    seekHandler = cb
    configureRemoteCommands()
}

/// Update Now Playing metadata. Called by Rust whenever playback state
/// changes. Pass title=NULL to clear (e.g. when stopping playback).
@_cdecl("yappy_now_playing_set")
public func yappy_now_playing_set(
    _ titlePtr: UnsafePointer<CChar>?,
    _ artistPtr: UnsafePointer<CChar>?,
    _ albumPtr: UnsafePointer<CChar>?,
    _ durationSecs: Double,
    _ positionSecs: Double,
    _ isPlaying: Bool
) {
    if titlePtr == nil {
        // Clear all metadata — kicks Yappy off the lock screen.
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
        try? AVAudioSession.sharedInstance().setActive(false, options: [.notifyOthersOnDeactivation])
        return
    }

    let title  = titlePtr.map { String(cString: $0) } ?? ""
    let artist = artistPtr.map { String(cString: $0) } ?? ""
    let album  = albumPtr.map { String(cString: $0) } ?? ""

    var info: [String: Any] = [:]
    info[MPMediaItemPropertyTitle]              = title
    if !artist.isEmpty { info[MPMediaItemPropertyArtist]  = artist }
    if !album.isEmpty  { info[MPMediaItemPropertyAlbumTitle] = album }
    info[MPMediaItemPropertyPlaybackDuration]   = max(0, durationSecs)
    info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = max(0, positionSecs)
    info[MPNowPlayingInfoPropertyPlaybackRate]  = isPlaying ? 1.0 : 0.0
    info[MPNowPlayingInfoPropertyMediaType]     = MPMediaType.audioBook.rawValue
    MPNowPlayingInfoCenter.default().nowPlayingInfo = info

    // Make sure the AVAudioSession is active so the system actually surfaces
    // these controls. The silent-keepalive code in AudioSession.swift uses
    // the same session — calling setActive(true) when it's already active is
    // a no-op.
    do {
        try AVAudioSession.sharedInstance().setCategory(.playback, mode: .spokenAudio, options: [])
        try AVAudioSession.sharedInstance().setActive(true)
    } catch {
        NSLog("[yappy/nowplaying] activate session failed: \(error)")
    }
}
