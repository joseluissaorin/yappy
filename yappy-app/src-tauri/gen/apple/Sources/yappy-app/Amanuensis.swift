// Amanuensis.swift: lo que el OCR de Yappy (Amanuensis Ligero, en Rust: capture/amanuensis.rs) pide al
// sistema en iOS. Rasterizar páginas de PDF escaneado (PDFKit), pasar a PNG las imágenes que Rust no
// decodifica (HEIC de las fotos; ImageIO con la orientación EXIF aplicada) y el OCR de Vision de respaldo,
// con la caja de cada renglón para rehacer los párrafos. Rust llama a estos símbolos por `extern "C"`.

import Foundation
import ImageIO
import PDFKit
import UIKit
import UniformTypeIdentifiers
import Vision

private func cadena(_ p: UnsafePointer<CChar>?) -> String? {
    guard let p = p else { return nil }
    return String(cString: p)
}

private func guardarPNG(_ img: CGImage, en ruta: String) -> Bool {
    let url = URL(fileURLWithPath: ruta) as CFURL
    guard let dest = CGImageDestinationCreateWithURL(url, UTType.png.identifier as CFString, 1, nil) else { return false }
    CGImageDestinationAddImage(dest, img, nil)
    return CGImageDestinationFinalize(dest)
}

@_cdecl("yappy_pdf_paginas")
public func yappy_pdf_paginas(_ ruta: UnsafePointer<CChar>?) -> Int32 {
    guard let r = cadena(ruta), let doc = PDFDocument(url: URL(fileURLWithPath: r)) else { return -1 }
    return Int32(doc.pageCount)
}

/// Una página del PDF a PNG sobre blanco, a `dpi` (lado mayor como mucho 4000 px). `thumbnail` dibuja la
/// página derecha (respeta /Rotate) y con fondo blanco.
@_cdecl("yappy_pdf_pagina_png")
public func yappy_pdf_pagina_png(_ ruta: UnsafePointer<CChar>?, _ indice: Int32, _ dpi: Int32,
                                 _ destino: UnsafePointer<CChar>?) -> Bool {
    guard let r = cadena(ruta), let d = cadena(destino),
          let doc = PDFDocument(url: URL(fileURLWithPath: r)),
          let pag = doc.page(at: Int(indice)) else { return false }
    return autoreleasepool {
        let caja = pag.bounds(for: .mediaBox)
        let girada = pag.rotation % 180 != 0
        let (pw, ph) = girada ? (caja.height, caja.width) : (caja.width, caja.height)
        var esc = CGFloat(dpi) / 72.0
        let lado = max(pw, ph) * esc
        if lado > 4000 { esc *= 4000 / lado }
        // thumbnail(of:) trabaja en puntos de pantalla: se divide por la escala del dispositivo
        let s = UIScreen.main.scale
        let tam = CGSize(width: (pw * esc / s).rounded(), height: (ph * esc / s).rounded())
        guard let img = pag.thumbnail(of: tam, for: .mediaBox).cgImage else { return false }
        return guardarPNG(img, en: d)
    }
}

/// Cualquier imagen que ImageIO lea (HEIC incluido) a PNG derecho, con el lado mayor acotado.
@_cdecl("yappy_imagen_a_png")
public func yappy_imagen_a_png(_ origen: UnsafePointer<CChar>?, _ destino: UnsafePointer<CChar>?,
                               _ ladoMax: Int32) -> Bool {
    guard let o = cadena(origen), let d = cadena(destino),
          let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: o) as CFURL, nil) else { return false }
    let opciones: [CFString: Any] = [
        kCGImageSourceCreateThumbnailFromImageAlways: true,
        kCGImageSourceCreateThumbnailWithTransform: true,  // orientación EXIF
        kCGImageSourceThumbnailMaxPixelSize: Int(ladoMax),
    ]
    return autoreleasepool {
        guard let img = CGImageSourceCreateThumbnailAtIndex(src, 0, opciones as CFDictionary) else { return false }
        return guardarPNG(img, en: d)
    }
}

/// Vision sobre una imagen: JSON `[{"t", "c", "x0", "y0", "x1", "y1"}]` en px con origen arriba a la izquierda
/// (nil si falla). Se libera con `yappy_free_string`.
@_cdecl("yappy_vision_lineas")
public func yappy_vision_lineas(_ ruta: UnsafePointer<CChar>?) -> UnsafeMutablePointer<CChar>? {
    guard let r = cadena(ruta),
          let src = CGImageSourceCreateWithURL(URL(fileURLWithPath: r) as CFURL, nil),
          let img = CGImageSourceCreateImageAtIndex(src, 0, nil) else { return nil }
    let W = Double(img.width), H = Double(img.height)
    var out: [[String: Any]] = []
    let req = VNRecognizeTextRequest()
    req.recognitionLevel = .accurate
    req.usesLanguageCorrection = true
    if #available(iOS 16.0, *) { req.automaticallyDetectsLanguage = true }
    do {
        try VNImageRequestHandler(cgImage: img, options: [:]).perform([req])
    } catch {
        return nil
    }
    for obs in req.results ?? [] {
        guard let s = obs.topCandidates(1).first else { continue }
        let b = obs.boundingBox
        out.append(["t": s.string, "c": Double(s.confidence),
                    "x0": Double(b.minX) * W, "y0": (1 - Double(b.maxY)) * H,
                    "x1": Double(b.maxX) * W, "y1": (1 - Double(b.minY)) * H])
    }
    guard let data = try? JSONSerialization.data(withJSONObject: out),
          let txt = String(data: data, encoding: .utf8) else { return nil }
    return strdup(txt)
}
