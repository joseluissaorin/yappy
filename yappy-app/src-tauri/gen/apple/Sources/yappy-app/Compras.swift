// LAS COMPRAS: RevenueCat en Swift, hablado desde Rust por el puente C de
// la casa (mismo patrón que NowPlaying/Haptics).
//
//   - Rust registra DOS callbacks al arrancar: `resultado(id, json)` para
//     las respuestas a peticiones numeradas, y `pro(bool)` para cada cambio
//     de la entitlement (compra, restauración, renovación, expiración).
//   - Cada petición (ofertas, comprar, restaurar, cliente) lleva su número;
//     el trabajo corre en una Task y contesta por `resultado` con un JSON
//     pequeño y estable (ver compras.rs para el contrato).
//   - `yappy_compras_es_pro()` lee la CACHÉ del SDK: sirve para el arranque
//     en frío sin esperar a la red.
//
// El SDK llega por Swift Package Manager (project.yml → purchases-ios-spm).

import Foundation
import RevenueCat

public typealias YappyComprasResultado = @convention(c) (UInt64, UnsafePointer<CChar>?) -> Void
public typealias YappyComprasPro = @convention(c) (Bool) -> Void

private var resultadoCb: YappyComprasResultado?
private var proCb: YappyComprasPro?
private var entitlementId = "yappy_parlanchín"
private var configurado = false

// ─── El oyente: RevenueCat avisa de cada cambio de cliente ───────────────
private final class OyenteCompras: NSObject, PurchasesDelegate {
    func purchases(_ purchases: Purchases, receivedUpdated customerInfo: CustomerInfo) {
        avisarPro(customerInfo)
    }
}
private let oyente = OyenteCompras()

private func esPro(_ info: CustomerInfo?) -> Bool {
    guard let info else { return false }
    return info.entitlements[entitlementId]?.isActive == true
}

private func avisarPro(_ info: CustomerInfo) {
    proCb?(esPro(info))
}

private let isoFecha: ISO8601DateFormatter = {
    let f = ISO8601DateFormatter()
    f.formatOptions = [.withInternetDateTime]
    return f
}()

private func limpiar(_ dict: [String: Any?]) -> [String: Any] {
    var out: [String: Any] = [:]
    for (k, v) in dict {
        if let v { out[k] = v } else { out[k] = NSNull() }
    }
    return out
}

private func responder(_ id: UInt64, _ obj: [String: Any?]) {
    let limpio = limpiar(obj)
    let datos = (try? JSONSerialization.data(withJSONObject: limpio, options: [])) ?? Data("{\"error\":\"json\"}".utf8)
    let texto = String(decoding: datos, as: UTF8.self)
    texto.withCString { p in
        resultadoCb?(id, p)
    }
}

private func iso(_ periodo: SubscriptionPeriod?) -> String? {
    guard let p = periodo else { return nil }
    switch p.unit {
    case .day: return "P\(p.value)D"
    case .week: return "P\(p.value)W"
    case .month: return "P\(p.value)M"
    case .year: return "P\(p.value)Y"
    @unknown default: return nil
    }
}

private func tipo(_ t: PackageType) -> String {
    switch t {
    case .monthly: return "mensual"
    case .annual: return "anual"
    case .lifetime: return "vida"
    case .weekly: return "semanal"
    case .twoMonth, .threeMonth, .sixMonth: return "otro"
    default: return "otro"
    }
}

private func describir(_ p: Package) -> [String: Any] {
    let sp = p.storeProduct
    var intro: [String: Any]? = nil
    if let d = sp.introductoryDiscount {
        let modo: String
        switch d.paymentMode {
        case .freeTrial: modo = "prueba"
        case .payUpFront: modo = "adelantado"
        case .payAsYouGo: modo = "por_periodo"
        @unknown default: modo = "otro"
        }
        intro = limpiar([
            "precio": d.localizedPriceString,
            "periodo": iso(d.subscriptionPeriod),
            "modo": modo,
        ])
    }
    return limpiar([
        "id": p.identifier,
        "tipo": tipo(p.packageType),
        "producto": sp.productIdentifier,
        "precio": sp.localizedPriceString,
        "precio_num": NSDecimalNumber(decimal: sp.price).doubleValue,
        "moneda": sp.currencyCode,
        "periodo": iso(sp.subscriptionPeriod),
        "titulo": sp.localizedTitle,
        "intro": intro,
    ])
}

private func describirCliente(_ info: CustomerInfo) -> [String: Any?] {
    let ent = info.entitlements[entitlementId]
    let periodo: String?
    switch ent?.periodType {
    case .some(.intro): periodo = "intro"
    case .some(.trial): periodo = "prueba"
    case .some(.normal): periodo = "normal"
    default: periodo = nil
    }
    return [
        "pro": ent?.isActive == true,
        "producto": ent?.productIdentifier,
        "desde": ent?.originalPurchaseDate.map { isoFecha.string(from: $0) },
        "expira": ent?.expirationDate.map { isoFecha.string(from: $0) },
        "renovara": ent?.willRenew,
        "periodo": periodo,
        "gestionar": info.managementURL?.absoluteString,
    ]
}

private func errorJson(_ error: Error) -> [String: Any?] {
    let ns = error as NSError
    return ["error": ns.localizedDescription, "codigo": ns.code]
}

// ─── PUBLIC FFI ──────────────────────────────────────────────────────────

@_cdecl("yappy_compras_register_resultado")
public func yappy_compras_register_resultado(_ cb: YappyComprasResultado?) {
    resultadoCb = cb
}

@_cdecl("yappy_compras_register_pro")
public func yappy_compras_register_pro(_ cb: YappyComprasPro?) {
    proCb = cb
}

@_cdecl("yappy_compras_configurar")
public func yappy_compras_configurar(_ apiKeyPtr: UnsafePointer<CChar>?, _ entlPtr: UnsafePointer<CChar>?) {
    guard let apiKeyPtr else { return }
    let apiKey = String(cString: apiKeyPtr)
    if let entlPtr { entitlementId = String(cString: entlPtr) }
    guard !configurado else { return }
    configurado = true
    Purchases.logLevel = .warn
    Purchases.configure(withAPIKey: apiKey)
    Purchases.shared.delegate = oyente
    // La verdad fresca en cuanto llegue (la caché ya contestó al arranque).
    Task {
        if let info = try? await Purchases.shared.customerInfo() {
            avisarPro(info)
        }
    }
}

@_cdecl("yappy_compras_es_pro")
public func yappy_compras_es_pro() -> Bool {
    guard configurado else { return false }
    return esPro(Purchases.shared.cachedCustomerInfo)
}

@_cdecl("yappy_compras_ofertas")
public func yappy_compras_ofertas(_ id: UInt64) {
    guard configurado else { responder(id, ["error": "tienda sin configurar"]); return }
    Task {
        do {
            let ofertas = try await Purchases.shared.offerings()
            let paquetes = (ofertas.current?.availablePackages ?? []).map(describir)
            responder(id, ["paquetes": paquetes, "oferta": ofertas.current?.identifier])
        } catch {
            responder(id, errorJson(error))
        }
    }
}

@_cdecl("yappy_compras_comprar")
public func yappy_compras_comprar(_ id: UInt64, _ paquetePtr: UnsafePointer<CChar>?) {
    guard configurado, let paquetePtr else { responder(id, ["error": "tienda sin configurar"]); return }
    let paqueteId = String(cString: paquetePtr)
    Task {
        do {
            let ofertas = try await Purchases.shared.offerings()
            guard let paquete = ofertas.current?.availablePackages.first(where: { $0.identifier == paqueteId }) else {
                responder(id, ["error": "paquete desconocido: \(paqueteId)"])
                return
            }
            let r = try await Purchases.shared.purchase(package: paquete)
            if r.userCancelled {
                responder(id, ["cancelado": true])
                return
            }
            let pro = esPro(r.customerInfo)
            proCb?(pro)
            responder(id, ["ok": true, "pro": pro, "producto": paquete.storeProduct.productIdentifier])
        } catch {
            let ns = error as NSError
            if ns.code == ErrorCode.purchaseCancelledError.rawValue {
                responder(id, ["cancelado": true])
            } else {
                responder(id, errorJson(error))
            }
        }
    }
}

@_cdecl("yappy_compras_restaurar")
public func yappy_compras_restaurar(_ id: UInt64) {
    guard configurado else { responder(id, ["error": "tienda sin configurar"]); return }
    Task {
        do {
            let info = try await Purchases.shared.restorePurchases()
            let pro = esPro(info)
            proCb?(pro)
            responder(id, ["ok": true, "pro": pro])
        } catch {
            responder(id, errorJson(error))
        }
    }
}

@_cdecl("yappy_compras_cliente")
public func yappy_compras_cliente(_ id: UInt64) {
    guard configurado else { responder(id, ["error": "tienda sin configurar"]); return }
    Task {
        do {
            let info = try await Purchases.shared.customerInfo()
            responder(id, describirCliente(info))
        } catch {
            responder(id, errorJson(error))
        }
    }
}

/// El identificador anónimo de RevenueCat ($RCAnonymousID…): sirve para
/// unir las estadísticas anónimas con el resultado de la cuerda.
@_cdecl("yappy_compras_usuario")
public func yappy_compras_usuario() -> UnsafeMutablePointer<CChar>? {
    guard configurado else { return nil }
    return strdup(Purchases.shared.appUserID)
}

@_cdecl("yappy_compras_gestionar")
public func yappy_compras_gestionar() {
    guard configurado else { return }
    Task { @MainActor in
        try? await Purchases.shared.showManageSubscriptions()
    }
}
