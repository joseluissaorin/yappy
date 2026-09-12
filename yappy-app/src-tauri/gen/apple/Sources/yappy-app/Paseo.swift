// EL PASEO DEL LORO: dos ayudas nativas para el onboarding.
//   - ¿Hay un enlace en el portapapeles? `hasURLs` es metadato: no lee el
//     contenido y no dispara el aviso de pegado de iOS.
//   - El loro en PiP: un AVPlayer con un vídeo mudo empaquetado, en una
//     capa pequeña sobre el webview (donde la página deja el hueco). Con
//     `canStartPictureInPictureAutomaticallyFromInline`, al irse el usuario
//     a Safari el vídeo se queda flotando: el loro señala «compartir».

import AVFoundation
import AVKit
import UIKit

@_cdecl("yappy_portapapeles_tiene_enlace")
public func yappy_portapapeles_tiene_enlace() -> Bool {
    UIPasteboard.general.hasURLs
}

private final class LoroPiP: NSObject, AVPictureInPictureControllerDelegate {
    var player: AVQueuePlayer?
    var looper: AVPlayerLooper?
    var vista: UIView?
    var capa: AVPlayerLayer?
    var pip: AVPictureInPictureController?

    func iniciar(path: String, marco: CGRect) {
        parar()
        let item = AVPlayerItem(url: URL(fileURLWithPath: path))
        let player = AVQueuePlayer()
        player.isMuted = true
        let looper = AVPlayerLooper(player: player, templateItem: item)
        let capa = AVPlayerLayer(player: player)
        capa.videoGravity = .resizeAspect
        capa.frame = CGRect(origin: .zero, size: marco.size)
        let vista = UIView(frame: marco)
        vista.backgroundColor = .clear
        vista.layer.addSublayer(capa)
        vista.isUserInteractionEnabled = false
        guard let ventana = UIApplication.shared.connectedScenes
            .compactMap({ $0 as? UIWindowScene })
            .flatMap({ $0.windows })
            .first(where: { $0.isKeyWindow }) else { return }
        ventana.addSubview(vista)
        if AVPictureInPictureController.isPictureInPictureSupported() {
            let pip = AVPictureInPictureController(playerLayer: capa)
            pip?.delegate = self
            pip?.canStartPictureInPictureAutomaticallyFromInline = true
            self.pip = pip
        }
        self.player = player
        self.looper = looper
        self.vista = vista
        self.capa = capa
        player.play()
    }

    func parar() {
        pip?.stopPictureInPicture()
        player?.pause()
        vista?.removeFromSuperview()
        player = nil
        looper = nil
        vista = nil
        capa = nil
        pip = nil
    }
}
private let loroPiP = LoroPiP()

@_cdecl("yappy_pip_iniciar")
public func yappy_pip_iniciar(_ pathPtr: UnsafePointer<CChar>?, _ x: Double, _ y: Double, _ w: Double, _ h: Double) {
    guard let pathPtr else { return }
    let path = String(cString: pathPtr)
    DispatchQueue.main.async {
        loroPiP.iniciar(path: path, marco: CGRect(x: x, y: y, width: w, height: h))
    }
}

@_cdecl("yappy_pip_parar")
public func yappy_pip_parar() {
    DispatchQueue.main.async {
        loroPiP.parar()
    }
}
