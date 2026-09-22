package com.yappy.app

import android.content.Intent
import android.net.Uri
import com.revenuecat.purchases.CustomerInfo
import com.revenuecat.purchases.LogLevel
import com.revenuecat.purchases.Offerings
import com.revenuecat.purchases.Package
import com.revenuecat.purchases.PackageType
import com.revenuecat.purchases.PeriodType
import com.revenuecat.purchases.PurchaseParams
import com.revenuecat.purchases.Purchases
import com.revenuecat.purchases.PurchasesConfiguration
import com.revenuecat.purchases.PurchasesError
import com.revenuecat.purchases.PurchasesErrorCode
import com.revenuecat.purchases.getCustomerInfoWith
import com.revenuecat.purchases.getOfferingsWith
import com.revenuecat.purchases.interfaces.UpdatedCustomerInfoListener
import com.revenuecat.purchases.models.StoreTransaction
import com.revenuecat.purchases.purchaseWith
import com.revenuecat.purchases.restorePurchasesWith
import org.json.JSONObject
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

// LAS COMPRAS: RevenueCat en Kotlin, hablado desde Rust por el puente
// (mismo contrato JSON que Compras.swift; ver compras.rs).
object Compras {
  private var entitlementId = "yappy_parlanchín"
  @Volatile private var configurado = false
  @Volatile private var ultimo: CustomerInfo? = null

  private val iso = SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'", Locale.US).apply { timeZone = TimeZone.getTimeZone("UTC") }

  private fun esPro(info: CustomerInfo?): Boolean = info?.entitlements?.get(entitlementId)?.isActive == true

  fun configurar(apiKey: String, entitlement: String) {
    entitlementId = entitlement
    if (configurado) return
    configurado = true
    Puente.principal.post {
      Purchases.logLevel = LogLevel.WARN
      Purchases.configure(PurchasesConfiguration.Builder(Puente.app, apiKey).build())
      Purchases.sharedInstance.updatedCustomerInfoListener = UpdatedCustomerInfoListener { info ->
        ultimo = info
        Puente.nativoPro(esPro(info))
      }
      // La verdad fresca en cuanto llegue (la caché ya contestó al arranque).
      Purchases.sharedInstance.getCustomerInfoWith({ }, { info -> ultimo = info; Puente.nativoPro(esPro(info)) })
    }
  }

  fun esProCache(): Boolean = configurado && esPro(ultimo)

  private fun responder(id: Long, obj: JSONObject) {
    Puente.nativoCompras(id, obj.toString())
  }

  private fun errorJson(e: PurchasesError): JSONObject =
    JSONObject().put("error", e.message).put("codigo", e.code.code).put("detalle", e.underlyingErrorMessage ?: JSONObject.NULL)

  private fun tipo(t: PackageType): String = when (t) {
    PackageType.MONTHLY -> "mensual"
    PackageType.ANNUAL -> "anual"
    PackageType.LIFETIME -> "vida"
    PackageType.WEEKLY -> "semanal"
    else -> "otro"
  }

  private fun describir(p: Package): JSONObject {
    val sp = p.product
    val o = JSONObject()
    o.put("id", p.identifier)
    o.put("tipo", tipo(p.packageType))
    o.put("producto", sp.id)
    o.put("precio", sp.price.formatted)
    o.put("precio_num", sp.price.amountMicros / 1_000_000.0)
    o.put("moneda", sp.price.currencyCode)
    o.put("periodo", sp.period?.iso8601 ?: JSONObject.NULL)
    o.put("titulo", sp.title)
    val intro = sp.defaultOption?.introPhase
    if (intro != null) {
      o.put("intro", JSONObject()
        .put("precio", intro.price.formatted)
        .put("periodo", intro.billingPeriod.iso8601)
        .put("modo", "por_periodo"))
    } else o.put("intro", JSONObject.NULL)
    return o
  }

  private fun describirCliente(info: CustomerInfo): JSONObject {
    val ent = info.entitlements[entitlementId]
    val o = JSONObject()
    o.put("pro", ent?.isActive == true)
    o.put("producto", ent?.productIdentifier ?: JSONObject.NULL)
    o.put("desde", ent?.originalPurchaseDate?.let { iso.format(it) } ?: JSONObject.NULL)
    o.put("expira", ent?.expirationDate?.let { iso.format(it) } ?: JSONObject.NULL)
    o.put("renovara", ent?.willRenew ?: JSONObject.NULL)
    o.put("periodo", when (ent?.periodType) {
      PeriodType.INTRO -> "intro"; PeriodType.TRIAL -> "prueba"; PeriodType.NORMAL -> "normal"; else -> JSONObject.NULL
    })
    o.put("gestionar", info.managementURL?.toString() ?: JSONObject.NULL)
    return o
  }

  fun ofertas(id: Long) {
    if (!configurado) { responder(id, JSONObject().put("error", "tienda sin configurar")); return }
    Puente.principal.post {
      Purchases.sharedInstance.getOfferingsWith(
        onError = { e -> responder(id, errorJson(e)) },
        onSuccess = { ofertas: Offerings ->
          val actual = ofertas.current
          val arr = org.json.JSONArray()
          actual?.availablePackages?.forEach { arr.put(describir(it)) }
          responder(id, JSONObject().put("paquetes", arr).put("oferta", actual?.identifier ?: JSONObject.NULL))
        }
      )
    }
  }

  fun comprar(id: Long, paqueteId: String) {
    if (!configurado) { responder(id, JSONObject().put("error", "tienda sin configurar")); return }
    Puente.principal.post {
      val actividad = Puente.actividad
      if (actividad == null) { responder(id, JSONObject().put("error", "sin pantalla para la tienda")); return@post }
      Purchases.sharedInstance.getOfferingsWith(
        onError = { e -> responder(id, errorJson(e)) },
        onSuccess = { ofertas ->
          val paquete = ofertas.current?.availablePackages?.firstOrNull { it.identifier == paqueteId }
          if (paquete == null) { responder(id, JSONObject().put("error", "paquete desconocido: $paqueteId")); return@getOfferingsWith }
          Purchases.sharedInstance.purchaseWith(
            PurchaseParams.Builder(actividad, paquete).build(),
            onError = { e, cancelado ->
              if (cancelado || e.code == PurchasesErrorCode.PurchaseCancelledError) responder(id, JSONObject().put("cancelado", true))
              else responder(id, errorJson(e))
            },
            onSuccess = { _: StoreTransaction?, info: CustomerInfo ->
              ultimo = info
              val pro = esPro(info)
              Puente.nativoPro(pro)
              responder(id, JSONObject().put("ok", true).put("pro", pro).put("producto", paquete.product.id))
            }
          )
        }
      )
    }
  }

  fun restaurar(id: Long) {
    if (!configurado) { responder(id, JSONObject().put("error", "tienda sin configurar")); return }
    Puente.principal.post {
      Purchases.sharedInstance.restorePurchasesWith(
        onError = { e -> responder(id, errorJson(e)) },
        onSuccess = { info ->
          ultimo = info
          val pro = esPro(info)
          Puente.nativoPro(pro)
          responder(id, JSONObject().put("ok", true).put("pro", pro))
        }
      )
    }
  }

  fun cliente(id: Long) {
    if (!configurado) { responder(id, JSONObject().put("error", "tienda sin configurar")); return }
    Puente.principal.post {
      Purchases.sharedInstance.getCustomerInfoWith(
        onError = { e -> responder(id, errorJson(e)) },
        onSuccess = { info -> ultimo = info; responder(id, describirCliente(info)) }
      )
    }
  }

  /** El identificador anónimo ($RCAnonymousID…) para unir las estadísticas
   *  anónimas con la cuerda. */
  fun usuario(): String? = if (configurado) try { Purchases.sharedInstance.appUserID } catch (_: Exception) { null } else null

  /** La página de suscripciones de Google Play (o la URL de gestión de RC). */
  fun gestionar() {
    Puente.principal.post {
      val a = Puente.actividad ?: return@post
      val url = ultimo?.managementURL?.toString()
        ?: "https://play.google.com/store/account/subscriptions?package=${a.packageName}"
      try { a.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url))) } catch (_: Exception) {}
    }
  }
}
