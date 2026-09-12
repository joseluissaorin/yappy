// LOS RECURSOS: las voces que llegan con la instalación (Background Assets)
// y dos consultas pequeñas que Rust necesita para decidir descargas:
//   - el contenedor del App Group (ahí deja las voces la extensión
//     YappyRecursos durante la instalación, y ahí las lee el motor);
//   - si la red es barata (wifi, no medida) para arrancar la descarga sola.
//
// Si el usuario abre la app antes de que la App Store termine las
// descargas esenciales, la app las sube a primer plano y publica el
// progreso a Rust por callback (el loro se las come en pantalla).

import BackgroundAssets
import Foundation
import Network

private let APP_GROUP = "group.com.joseluissaorin.yappy"

public typealias YappyProgresoCallback = @convention(c) (UInt64, UInt64, Bool) -> Void
private var progresoCb: YappyProgresoCallback?

@_cdecl("yappy_app_group_path")
public func yappy_app_group_path() -> UnsafeMutablePointer<CChar>? {
    guard let url = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: APP_GROUP) else {
        return nil
    }
    return strdup(url.path)
}

// ─── La red ──────────────────────────────────────────────────────────────
// El monitor tarda unos milisegundos en dar su primera lectura; hasta
// entonces `currentPath` dice «sin red». La primera consulta espera a esa
// lectura (con techo) para no confundir «aún no sé» con «red cara».
private let primeraLectura = DispatchSemaphore(value: 0)
private var redLeida = false
private let monitorRed: NWPathMonitor = {
    let m = NWPathMonitor()
    m.pathUpdateHandler = { _ in
        if !redLeida {
            redLeida = true
            primeraLectura.signal()
        }
    }
    m.start(queue: DispatchQueue(label: "yappy.red", qos: .utility))
    return m
}()

@_cdecl("yappy_red_barata")
public func yappy_red_barata() -> Bool {
    let m = monitorRed
    if !redLeida {
        _ = primeraLectura.wait(timeout: .now() + 2.0)
    }
    let p = m.currentPath
    return p.status == .satisfied && !p.isExpensive && !p.isConstrained
}

// ─── Background Assets desde la app ──────────────────────────────────────
// Solo iOS 16.4+ tiene descargas esenciales; el mínimo de Yappy es 17.
private final class OyenteDescargas: NSObject, BADownloadManagerDelegate {
    var esperados: [String: Int64] = [:]
    var escritos: [String: Int64] = [:]

    func publicar(fin: Bool) {
        let done = escritos.values.reduce(0, +)
        let total = max(esperados.values.reduce(0, +), done)
        progresoCb?(UInt64(max(0, done)), UInt64(max(0, total)), fin)
    }
    func download(_ download: BADownload, didWriteBytes bytesWritten: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite totalExpectedBytes: Int64) {
        escritos[download.identifier] = totalBytesWritten
        esperados[download.identifier] = totalExpectedBytes
        publicar(fin: false)
    }
    func download(_ download: BADownload, finishedWithFileURL fileURL: URL) {
        // La extensión es quien coloca el fichero: aquí solo se anota que
        // este ya llegó. Si la app recibe el fichero (descarga en primer
        // plano), lo coloca ella misma en el contenedor compartido.
        colocar(download: download, fileURL: fileURL)
        escritos[download.identifier] = esperados[download.identifier] ?? escritos[download.identifier] ?? 0
        publicar(fin: false)
        comprobarFin()
    }
    func download(_ download: BADownload, failedWithError error: any Error) {
        NSLog("yappy recursos: descarga %@ falló: %@", download.identifier, "\(error)")
        comprobarFin()
    }
    func comprobarFin() {
        BADownloadManager.shared.fetchCurrentDownloads { downloads, _ in
            if downloads.isEmpty {
                DispatchQueue.main.async { self.publicar(fin: true) }
            }
        }
    }
}
private let oyente = OyenteDescargas()

/// Coloca un fichero descargado en su sitio del contenedor compartido. El
/// identificador de la descarga ES la ruta relativa dentro del modelo
/// («onnx/vocoder.onnx»), así que no hace falta manifiesto para colocarlo.
func colocar(download: BADownload, fileURL: URL) {
    guard let base = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: APP_GROUP) else { return }
    let destino = base.appendingPathComponent("models/supertonic-3").appendingPathComponent(download.identifier)
    do {
        try FileManager.default.createDirectory(at: destino.deletingLastPathComponent(), withIntermediateDirectories: true)
        if FileManager.default.fileExists(atPath: destino.path) {
            try FileManager.default.removeItem(at: destino)
        }
        try FileManager.default.moveItem(at: fileURL, to: destino)
    } catch {
        NSLog("yappy recursos: no pude colocar %@: %@", download.identifier, "\(error)")
    }
}

@_cdecl("yappy_ba_register_progreso")
public func yappy_ba_register_progreso(_ cb: YappyProgresoCallback?) {
    progresoCb = cb
}

/// Si hay descargas de la instalación en marcha, las sube a primer plano y
/// empieza a publicar progreso. Devuelve cuántas había.
@_cdecl("yappy_ba_reanudar")
public func yappy_ba_reanudar() -> Int32 {
    let manager = BADownloadManager.shared
    manager.delegate = oyente
    let grupo = DispatchGroup()
    var cuenta: Int32 = 0
    grupo.enter()
    manager.fetchCurrentDownloads { downloads, error in
        if let error { NSLog("yappy recursos: fetchCurrentDownloads: %@", "\(error)") }
        cuenta = Int32(downloads.count)
        for d in downloads {
            oyente.esperados[d.identifier] = oyente.esperados[d.identifier] ?? 0
            do {
                try manager.startForegroundDownload(d)
            } catch {
                NSLog("yappy recursos: no pude subir a primer plano %@: %@", d.identifier, "\(error)")
            }
        }
        grupo.leave()
    }
    _ = grupo.wait(timeout: .now() + 3)
    return cuenta
}
