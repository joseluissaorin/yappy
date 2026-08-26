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

/// Activate the audio session for normal TTS playback. cpal/CoreAudio only
/// outputs sound on iOS when an AVAudioSession is active in .playback category;
/// without this, streaming TTS plays silently. Called once at playback startup.
/// Synchronous (not a Task) so the session is active before cpal builds its
/// output stream. Audio hablado estandar: interrumpe a otras apps (que
/// pausan y luego reanudan), en vez de dejarlas sonando por debajo.
@_cdecl("yappy_audio_session_activate")
public func yappy_audio_session_activate() {
    let session = AVAudioSession.sharedInstance()
    do {
        try session.setCategory(.playback, mode: .spokenAudio, options: [])
        try session.setActive(true)
    } catch {
        NSLog("[yappy/audio] playback session activate failed: \(error)")
    }
}

// ─── INTERRUPCIONES DEL MUNDO REAL ──────────────────────────────────────
//
// Llamada entrante, Siri, otra app tomando el audio → pausa inmediata.
// Auriculares desconectados → pausa inmediata (la ley de oro del audio
// móvil: nadie quiere su artículo sonando por el altavoz del vagón).
// Rust registra un callback: cb(false) = pausa YA; cb(true) = el sistema
// dice explícitamente que la interrupción acabó y procede reanudar.

public typealias YappyInterruptionCallback = @convention(c) (Bool) -> Void
private var interruptionHandler: YappyInterruptionCallback?
private var interruptionObserversInstalled = false

@_cdecl("yappy_register_interruption_handler")
public func yappy_register_interruption_handler(_ cb: YappyInterruptionCallback?) {
    interruptionHandler = cb
    guard !interruptionObserversInstalled else { return }
    interruptionObserversInstalled = true

    let nc = NotificationCenter.default
    nc.addObserver(
        forName: AVAudioSession.interruptionNotification,
        object: AVAudioSession.sharedInstance(),
        queue: .main
    ) { note in
        guard let info = note.userInfo,
              let typeRaw = info[AVAudioSessionInterruptionTypeKey] as? UInt,
              let type = AVAudioSession.InterruptionType(rawValue: typeRaw)
        else { return }
        switch type {
        case .began:
            NSLog("[yappy/audio] interrupción: comenzó → pausa")
            interruptionHandler?(false)
        case .ended:
            let optsRaw = info[AVAudioSessionInterruptionOptionKey] as? UInt ?? 0
            let opts = AVAudioSession.InterruptionOptions(rawValue: optsRaw)
            if opts.contains(.shouldResume) {
                NSLog("[yappy/audio] interrupción: terminó con shouldResume → reanudar")
                // Reactivar la sesión antes de reanudar: el sistema pudo
                // desactivarla durante la interrupción.
                try? AVAudioSession.sharedInstance().setActive(true)
                interruptionHandler?(true)
            } else {
                NSLog("[yappy/audio] interrupción: terminó sin shouldResume → seguimos en pausa")
            }
        @unknown default:
            break
        }
    }
    nc.addObserver(
        forName: AVAudioSession.routeChangeNotification,
        object: AVAudioSession.sharedInstance(),
        queue: .main
    ) { note in
        guard let info = note.userInfo,
              let reasonRaw = info[AVAudioSessionRouteChangeReasonKey] as? UInt,
              let reason = AVAudioSession.RouteChangeReason(rawValue: reasonRaw)
        else { return }
        if reason == .oldDeviceUnavailable {
            NSLog("[yappy/audio] auriculares fuera → pausa")
            interruptionHandler?(false)
        }
    }
    NSLog("[yappy/audio] observadores de interrupción instalados")
}

// ─── EL CANAL DE EFECTOS ────────────────────────────────────────────────
//
// Un AVAudioPlayer aparte para sonidos cortos locales (muestras de voz,
// foley). NO toca la sesión de lectura: el documento en pausa sigue en
// pausa, y el Now Playing no se entera.

private var efectoPlayer: AVAudioPlayer?

@_cdecl("yappy_efecto_play")
public func yappy_efecto_play(_ pathPtr: UnsafePointer<CChar>?) -> Double {
    guard let pathPtr = pathPtr else { return 0.0 }
    let path = String(cString: pathPtr)
    let url = URL(fileURLWithPath: path)
    // La sesión debe estar activa para que suene también con la app recién
    // abierta; .playback ya es la categoría de la casa.
    try? AVAudioSession.sharedInstance().setActive(true)
    do {
        let player = try AVAudioPlayer(contentsOf: url)
        efectoPlayer?.stop()
        efectoPlayer = player
        player.prepareToPlay()
        player.play()
        return player.duration
    } catch {
        NSLog("[yappy/efecto] no se pudo reproducir \(path): \(error)")
        return 0.0
    }
}

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

private let APP_GROUP = "group.com.joseluissaorin.yappy"

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
