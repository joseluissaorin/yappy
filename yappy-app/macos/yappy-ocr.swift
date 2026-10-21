// yappy-ocr.swift — Vision-framework OCR helper.
// Usage:  swift yappy-ocr.swift <image-path> [primary-lang] [json]
// Prints recognized text (paragraph-segmented by line breaks) to stdout.
// With «json»: one object per line of text, [{"t", "c", "x0", "y0", "x1", "y1"}] in pixels with the origin at
// the top left, so Yappy can rebuild paragraphs from the geometry (Amanuensis fallback).

import AppKit
import Foundation
import Vision

guard CommandLine.arguments.count >= 2 else {
    FileHandle.standardError.write("usage: yappy-ocr <image-path> [lang]\n".data(using: .utf8)!)
    exit(2)
}

let path = CommandLine.arguments[1]
let lang = CommandLine.arguments.count > 2 ? CommandLine.arguments[2] : "en-US"
let asJSON = CommandLine.arguments.count > 3 && CommandLine.arguments[3] == "json"

let url = URL(fileURLWithPath: path)
guard let image = NSImage(contentsOf: url),
      let tiff = image.tiffRepresentation,
      let bitmap = NSBitmapImageRep(data: tiff),
      let cgImage = bitmap.cgImage else {
    FileHandle.standardError.write("could not read image at \(path)\n".data(using: .utf8)!)
    exit(3)
}

let request = VNRecognizeTextRequest { (req, err) in
    guard err == nil else {
        FileHandle.standardError.write("OCR error: \(err!)\n".data(using: .utf8)!)
        exit(4)
    }
    guard let results = req.results as? [VNRecognizedTextObservation] else {
        return
    }
    if asJSON {
        let W = Double(cgImage.width), H = Double(cgImage.height)
        var out: [[String: Any]] = []
        for obs in results {
            guard let s = obs.topCandidates(1).first else { continue }
            let b = obs.boundingBox  // normalized, origin bottom-left
            out.append(["t": s.string, "c": Double(s.confidence),
                        "x0": Double(b.minX) * W, "y0": (1 - Double(b.maxY)) * H,
                        "x1": Double(b.maxX) * W, "y1": (1 - Double(b.minY)) * H])
        }
        if let d = try? JSONSerialization.data(withJSONObject: out), let txt = String(data: d, encoding: .utf8) {
            print(txt)
        }
        return
    }
    var lines: [String] = []
    for obs in results {
        if let s = obs.topCandidates(1).first {
            lines.append(s.string)
        }
    }
    print(lines.joined(separator: "\n"))
}
request.recognitionLevel = .accurate
request.usesLanguageCorrection = true
if #available(macOS 13.0, *) {
    request.automaticallyDetectsLanguage = true
}
request.recognitionLanguages = [lang, "en-US"]
let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
do {
    try handler.perform([request])
} catch {
    FileHandle.standardError.write("perform failed: \(error)\n".data(using: .utf8)!)
    exit(5)
}
