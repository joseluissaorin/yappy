// LA EXTENSIÓN DE RECURSOS: Background Assets. La App Store la lanza
// durante la instalación (y en cada actualización): lee nuestro
// manifiesto, encola las voces como descargas ESENCIALES (cuentan en la
// barra de instalación) y, cuando cada fichero llega, lo coloca en el
// contenedor del App Group, que es donde el motor de Yappy las busca.
//
// El identificador de cada descarga es su ruta dentro del modelo
// («onnx/vocoder.onnx»): con eso se coloca sin releer el manifiesto.

import BackgroundAssets
import Foundation

private let APP_GROUP = "group.com.joseluissaorin.yappy"

struct Manifiesto: Decodable {
    struct Fichero: Decodable {
        let id: String
        let url: String
        let bytes: Int
        let esencial: Bool
    }
    let version: Int
    let variante: String
    let ficheros: [Fichero]
}

private func raizModelo() -> URL? {
    FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: APP_GROUP)?
        .appendingPathComponent("models/supertonic-3")
}

@main
struct Descargador: BADownloaderExtension {
    func downloads(for request: BAContentRequest, manifestURL: URL, extensionInfo: BAAppExtensionInfo) -> Set<BADownload> {
        let manifiesto: Manifiesto
        do {
            let datos = try Data(contentsOf: manifestURL)
            manifiesto = try JSONDecoder().decode(Manifiesto.self, from: datos)
        } catch {
            NSLog("yappy recursos: manifiesto ilegible: %@", "\(error)")
            return []
        }
        // Las esenciales solo caben en la instalación o la actualización.
        let esencialesPermitidas = request == .install || request == .update
        let raiz = raizModelo()
        var descargas: Set<BADownload> = []
        for f in manifiesto.ficheros {
            // Lo que ya está en su sitio con el tamaño esperado no se repite.
            if let raiz, let attrs = try? FileManager.default.attributesOfItem(atPath: raiz.appendingPathComponent(f.id).path),
               let tam = attrs[.size] as? Int, tam == f.bytes {
                continue
            }
            guard let url = URL(string: f.url) else { continue }
            let d = BAURLDownload(
                identifier: f.id,
                request: URLRequest(url: url),
                essential: f.esencial && esencialesPermitidas,
                fileSize: f.bytes,
                applicationGroupIdentifier: APP_GROUP,
                priority: .default)
            descargas.insert(d)
        }
        NSLog("yappy recursos: %d descargas encoladas (%@, %@)", descargas.count, "\(request)", manifiesto.variante)
        return descargas
    }

    func backgroundDownload(_ failedDownload: BADownload, failedWithError error: any Error) {
        guard type(of: failedDownload) == BAURLDownload.self else { return }
        // Una esencial que falla se reencola como normal para más tarde.
        if failedDownload.isEssential {
            do {
                try BADownloadManager.shared.scheduleDownload(failedDownload.removingEssential())
            } catch {
                NSLog("yappy recursos: no pude reencolar %@: %@", failedDownload.identifier, "\(error)")
            }
        } else {
            NSLog("yappy recursos: falló %@: %@", failedDownload.identifier, "\(error)")
        }
    }

    func backgroundDownload(_ finishedDownload: BADownload, finishedWithFileURL fileURL: URL) {
        guard let raiz = raizModelo() else { return }
        let destino = raiz.appendingPathComponent(finishedDownload.identifier)
        do {
            try FileManager.default.createDirectory(at: destino.deletingLastPathComponent(), withIntermediateDirectories: true)
            if FileManager.default.fileExists(atPath: destino.path) {
                try FileManager.default.removeItem(at: destino)
            }
            try FileManager.default.moveItem(at: fileURL, to: destino)
            NSLog("yappy recursos: colocado %@", finishedDownload.identifier)
        } catch {
            NSLog("yappy recursos: no pude colocar %@: %@", finishedDownload.identifier, "\(error)")
        }
    }

    func backgroundDownload(_ download: BADownload, didReceive challenge: URLAuthenticationChallenge) async -> (URLSession.AuthChallengeDisposition, URLCredential?) {
        (.performDefaultHandling, nil)
    }
}
