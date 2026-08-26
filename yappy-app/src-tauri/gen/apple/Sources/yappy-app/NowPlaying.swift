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
import WidgetKit
import AVFoundation
import UIKit

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
    // Adelante/atrás con la MISMA semántica que dentro de la app: una FRASE
    // (los flechazos de pista, honestos), no un «±15 s» que aquí mentiría.
    // Los AirPods (doble/triple apretón) llegan por estos mismos comandos.
    center.nextTrackCommand.isEnabled = true
    center.nextTrackCommand.addTarget { _ in
        skipForwardHandler?()
        return .success
    }
    center.previousTrackCommand.isEnabled = true
    center.previousTrackCommand.addTarget { _ in
        skipBackwardHandler?()
        return .success
    }
    center.skipForwardCommand.isEnabled = false
    center.skipBackwardCommand.isEnabled = false
    // Scrubbing (drag the lock-screen progress bar).
    center.changePlaybackPositionCommand.addTarget { event in
        guard let e = event as? MPChangePlaybackPositionCommandEvent else {
            return .commandFailed
        }
        seekHandler?(e.positionTime)
        return .success
    }

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
        // IMPORTANT: do NOT setActive(false) here. This is called on every
        // playback snapshot where duration < 0.1s — including the brief moment
        // right as "read aloud" starts, before the first chunk is synthesized.
        // Deactivating the shared AVAudioSession at that instant silences cpal's
        // output unit (it relies on an active session), so live TTS playback
        // would stay frozen at 0:00. Just clear the Now Playing info; the
        // PlaybackController owns the session lifecycle.
        MPNowPlayingInfoCenter.default().nowPlayingInfo = nil
        return
    }

    let title  = titlePtr.map { String(cString: $0) } ?? ""
    let artist = artistPtr.map { String(cString: $0) } ?? ""
    let album  = albumPtr.map { String(cString: $0) } ?? ""

    var info: [String: Any] = [:]
    info[MPMediaItemPropertyTitle]              = title
    if !artist.isEmpty { info[MPMediaItemPropertyArtist]  = artist }
    if !album.isEmpty  { info[MPMediaItemPropertyAlbumTitle] = album }
    // La carátula: el loro de la casa en la pantalla de bloqueo.
    if let img = UIImage(named: "LoroCaratula") {
        info[MPMediaItemPropertyArtwork] = MPMediaItemArtwork(boundsSize: img.size) { _ in img }
    }
    info[MPMediaItemPropertyPlaybackDuration]   = max(0, durationSecs)
    info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = max(0, positionSecs)
    info[MPNowPlayingInfoPropertyPlaybackRate]  = isPlaying ? 1.0 : 0.0
    info[MPNowPlayingInfoPropertyMediaType]     = MPMediaType.audioBook.rawValue
    MPNowPlayingInfoCenter.default().nowPlayingInfo = info

    // La sesion la configura UN solo sitio (AudioSession.swift); aqui solo
    // se asegura activa para que el sistema muestre los controles.
    do {
        try AVAudioSession.sharedInstance().setActive(true)
    } catch {
        NSLog("[yappy/nowplaying] activate session failed: \(error)")
    }

    // El widget de inicio lee este titulo del App Group; sin esta escritura
    // se quedaba para siempre en el texto generico.
    if let defaults = UserDefaults(suiteName: "group.com.joseluissaorin.yappy") {
        defaults.set(title, forKey: "last_played_title")
    }
    if #available(iOS 14.0, *) {
        WidgetCenter.shared.reloadTimelines(ofKind: "YappyHomeWidget")
    }
}
