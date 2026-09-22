package com.yappy.app

import android.app.Activity
import android.app.Application
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.media.MediaCodec
import android.media.MediaExtractor
import android.media.MediaFormat
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.VibrationEffect
import android.os.Vibrator
import android.os.VibratorManager
import android.view.textclassifier.TextClassifier
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import java.io.File
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder

// EL PUENTE: la misma superficie que los @_cdecl de Swift, en Kotlin. Rust
// llama a estos métodos estáticos por JNI (mobile_android.rs) y Kotlin le
// contesta por los tres `external fun` de abajo, que Rust exporta como
// Java_com_yappy_app_Puente_nativo*. Todo lo que toca vistas o sesiones va
// al hilo principal; lo demás corre donde le llamen.
object Puente {
  const val CANAL_AVISOS = "yappy.avisos"
  const val CANAL_LECTURA = "yappy.lectura"
  const val CANAL_IMPRENTA = "yappy.imprenta"

  lateinit var app: Application
  @Volatile var actividad: Activity? = null
  val principal = Handler(Looper.getMainLooper())

  fun ctx(): Context = actividad ?: app

  @Volatile private var contextoDado = false

  fun arrancar(a: Activity) {
    if (!::app.isInitialized) app = a.application
    actividad = a
    canales()
    // Rust necesita la JavaVM y un Context desde el primer momento (Tauri
    // no rellena ndk-context a tiempo en esta app): se los damos aquí.
    if (!contextoDado) { contextoDado = true; nativoContexto(app) }
  }

  /** La JavaVM y la Application para el lado Rust (ndk-context). */
  @JvmStatic external fun nativoContexto(app: Application)

  // ─── Kotlin → Rust ──────────────────────────────────────────────────────
  /** Mandos de la sesión de medios y del foco de audio: play, pause, toggle,
   *  next, prev, seek (valor = segundos), interrupcion (valor 0 = pausa ya,
   *  1 = terminó y procede reanudar). */
  @JvmStatic external fun nativoMando(cual: String, valor: Double)
  /** Respuesta a una petición numerada de la tienda (JSON). */
  @JvmStatic external fun nativoCompras(id: Long, json: String)
  /** Cada cambio de la cuerda (entitlement). */
  @JvmStatic external fun nativoPro(pro: Boolean)

  // ─── Avisos ─────────────────────────────────────────────────────────────
  private fun canales() {
    if (Build.VERSION.SDK_INT < 26) return
    val nm = app.getSystemService(NotificationManager::class.java)
    nm.createNotificationChannel(
      NotificationChannel(CANAL_AVISOS, "Avisos", NotificationManager.IMPORTANCE_DEFAULT).apply {
        description = "El audiolibro que termina mientras el teléfono duerme"
      }
    )
    nm.createNotificationChannel(
      NotificationChannel(CANAL_LECTURA, "Lectura", NotificationManager.IMPORTANCE_LOW).apply {
        description = "Los mandos de lo que suena"
        setShowBadge(false)
      }
    )
    nm.createNotificationChannel(
      NotificationChannel(CANAL_IMPRENTA, "Imprenta", NotificationManager.IMPORTANCE_LOW).apply {
        description = "El progreso de los audiolibros en marcha"
        setShowBadge(false)
      }
    )
  }

  private fun avisosConcedidos(): Boolean =
    Build.VERSION.SDK_INT < 33 ||
      ContextCompat.checkSelfPermission(app, "android.permission.POST_NOTIFICATIONS") == PackageManager.PERMISSION_GRANTED

  @JvmStatic
  fun notificar(id: String, titulo: String, cuerpo: String) {
    if (!avisosConcedidos()) return
    val abrir = Intent(app, MainActivity::class.java).apply { flags = Intent.FLAG_ACTIVITY_SINGLE_TOP }
    val pi = android.app.PendingIntent.getActivity(
      app, id.hashCode(), abrir,
      android.app.PendingIntent.FLAG_UPDATE_CURRENT or android.app.PendingIntent.FLAG_IMMUTABLE
    )
    val n = NotificationCompat.Builder(app, CANAL_AVISOS)
      .setSmallIcon(R.drawable.ic_yappy_aviso)
      .setContentTitle(titulo)
      .setContentText(cuerpo)
      .setStyle(NotificationCompat.BigTextStyle().bigText(cuerpo))
      .setContentIntent(pi)
      .setAutoCancel(true)
      .setPriority(NotificationCompat.PRIORITY_DEFAULT)
      .build()
    app.getSystemService(NotificationManager::class.java).notify(id.hashCode(), n)
  }

  /** El paseo pide el permiso a las claras (Android 13+); antes no existe. */
  @JvmStatic
  fun avisosPedir() {
    if (Build.VERSION.SDK_INT < 33) return
    val a = actividad ?: return
    principal.post {
      a.requestPermissions(arrayOf("android.permission.POST_NOTIFICATIONS"), 7101)
    }
  }

  /** 0 sin decidir · 1 concedido · 2 denegado. */
  @JvmStatic
  fun avisosEstado(): Int {
    if (avisosConcedidos()) return 1
    val a = actividad ?: return 0
    val preguntado = app.getSharedPreferences("yappy", Context.MODE_PRIVATE).getBoolean("avisos_preguntado", false)
    return if (preguntado || a.shouldShowRequestPermissionRationale("android.permission.POST_NOTIFICATIONS")) 2 else 0
  }

  fun avisosRespondido() {
    app.getSharedPreferences("yappy", Context.MODE_PRIVATE).edit().putBoolean("avisos_preguntado", true).apply()
  }

  // ─── Háptica ────────────────────────────────────────────────────────────
  private fun vibrador(): Vibrator? =
    if (Build.VERSION.SDK_INT >= 31) (app.getSystemService(Context.VIBRATOR_MANAGER_SERVICE) as? VibratorManager)?.defaultVibrator
    else @Suppress("DEPRECATION") app.getSystemService(Context.VIBRATOR_SERVICE) as? Vibrator

  @JvmStatic
  fun haptic(kind: String) {
    val v = vibrador() ?: return
    if (!v.hasVibrator()) return
    if (Build.VERSION.SDK_INT >= 29) {
      val efecto = when (kind) {
        "tick", "selection" -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_TICK)
        "light", "soft" -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_CLICK)
        "medium", "rigid" -> VibrationEffect.createPredefined(VibrationEffect.EFFECT_HEAVY_CLICK)
        "heavy" -> VibrationEffect.createWaveform(longArrayOf(0, 18, 30, 10), intArrayOf(0, 255, 0, 160), -1)
        "success" -> VibrationEffect.createWaveform(longArrayOf(0, 10, 60, 14), intArrayOf(0, 140, 0, 255), -1)
        "warning" -> VibrationEffect.createWaveform(longArrayOf(0, 14, 50, 14), intArrayOf(0, 200, 0, 200), -1)
        "error" -> VibrationEffect.createWaveform(longArrayOf(0, 12, 40, 12, 40, 12), intArrayOf(0, 255, 0, 255, 0, 255), -1)
        else -> return
      }
      v.vibrate(efecto)
    } else {
      val ms = when (kind) {
        "tick", "selection" -> 4L; "light", "soft" -> 8L; "medium", "rigid" -> 14L
        "heavy" -> 22L; "success" -> 20L; "warning" -> 24L; "error" -> 36L
        else -> return
      }
      if (Build.VERSION.SDK_INT >= 26) v.vibrate(VibrationEffect.createOneShot(ms, VibrationEffect.DEFAULT_AMPLITUDE))
      else @Suppress("DEPRECATION") v.vibrate(ms)
    }
  }

  // ─── Compartir hacia fuera ──────────────────────────────────────────────
  @JvmStatic
  fun compartirFichero(path: String) {
    val f = File(path)
    if (!f.exists()) return
    principal.post {
      val a = actividad ?: return@post
      try {
        val uri = FileProvider.getUriForFile(a, "${a.packageName}.fileprovider", f)
        val tipo = when (f.extension.lowercase()) {
          "m4b", "m4a" -> "audio/mp4"
          "mp3" -> "audio/mpeg"
          "wav" -> "audio/wav"
          "yappy" -> "application/zip"
          "txt", "md" -> "text/plain"
          else -> "application/octet-stream"
        }
        val enviar = Intent(Intent.ACTION_SEND).apply {
          type = tipo
          putExtra(Intent.EXTRA_STREAM, uri)
          putExtra(Intent.EXTRA_SUBJECT, f.nameWithoutExtension)
          addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        a.startActivity(Intent.createChooser(enviar, f.nameWithoutExtension))
      } catch (e: Exception) {
        android.util.Log.w("yappy", "compartir: $e")
      }
    }
  }

  // ─── Portapapeles ───────────────────────────────────────────────────────
  /** El texto copiado, para el gesto EXPLÍCITO de «pegar lo copiado». */
  @JvmStatic
  fun portapapelesTexto(): String? {
    val cm = app.getSystemService(ClipboardManager::class.java) ?: return null
    return try {
      val clip = cm.primaryClip ?: return null
      if (clip.itemCount == 0) return null
      clip.getItemAt(0).coerceToText(app)?.toString()?.takeIf { it.isNotBlank() }
    } catch (e: Exception) { null }
  }

  /** ¿Hay un enlace copiado? Sin leer el contenido: la descripción del clip
   *  (y su clasificación, en Android 12+) no dispara el aviso de pegado. */
  @JvmStatic
  fun portapapelesTieneEnlace(): Boolean {
    val cm = app.getSystemService(ClipboardManager::class.java) ?: return false
    return try {
      val d: ClipDescription = cm.primaryClipDescription ?: return false
      if (d.hasMimeType(ClipDescription.MIMETYPE_TEXT_URILIST)) return true
      if (Build.VERSION.SDK_INT >= 31 && d.classificationStatus == ClipDescription.CLASSIFICATION_COMPLETE) {
        return d.getConfidenceScore(TextClassifier.TYPE_URL) > 0.5f
      }
      false
    } catch (e: Exception) { false }
  }

  // ─── Audio de entrada: cualquier formato → WAV PCM16 ────────────────────
  /** Las notas de voz de WhatsApp llegan en Ogg-Opus, que el decodificador
   *  puro de Rust no lee en Android; MediaCodec sí. Devuelve true si escribió
   *  el WAV. */
  @JvmStatic
  fun decodificarAudio(origen: String, destinoWav: String): Boolean {
    val ex = MediaExtractor()
    var codec: MediaCodec? = null
    var salida: RandomAccessFile? = null
    try {
      ex.setDataSource(origen)
      var pista = -1
      var formato: MediaFormat? = null
      for (i in 0 until ex.trackCount) {
        val f = ex.getTrackFormat(i)
        if (f.getString(MediaFormat.KEY_MIME)?.startsWith("audio/") == true) { pista = i; formato = f; break }
      }
      if (pista < 0 || formato == null) return false
      ex.selectTrack(pista)
      val mime = formato.getString(MediaFormat.KEY_MIME)!!
      codec = MediaCodec.createDecoderByType(mime)
      codec.configure(formato, null, null, 0)
      codec.start()
      salida = RandomAccessFile(File(destinoWav), "rw")
      salida.setLength(0)
      salida.write(ByteArray(44)) // cabecera al final
      var canales = formato.getInteger(MediaFormat.KEY_CHANNEL_COUNT)
      var tasa = formato.getInteger(MediaFormat.KEY_SAMPLE_RATE)
      var bytes = 0L
      val info = MediaCodec.BufferInfo()
      var entradaHecha = false
      var salidaHecha = false
      while (!salidaHecha) {
        if (!entradaHecha) {
          val ib = codec.dequeueInputBuffer(10_000)
          if (ib >= 0) {
            val buf = codec.getInputBuffer(ib)!!
            val n = ex.readSampleData(buf, 0)
            if (n < 0) {
              codec.queueInputBuffer(ib, 0, 0, 0, MediaCodec.BUFFER_FLAG_END_OF_STREAM); entradaHecha = true
            } else {
              codec.queueInputBuffer(ib, 0, n, ex.sampleTime, 0); ex.advance()
            }
          }
        }
        val ob = codec.dequeueOutputBuffer(info, 10_000)
        when {
          ob >= 0 -> {
            val buf = codec.getOutputBuffer(ob)!!
            if (info.size > 0) {
              val trozo = ByteArray(info.size)
              buf.position(info.offset); buf.get(trozo, 0, info.size)
              salida.write(trozo); bytes += info.size
            }
            codec.releaseOutputBuffer(ob, false)
            if (info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) salidaHecha = true
          }
          ob == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> {
            val f = codec.outputFormat
            canales = f.getInteger(MediaFormat.KEY_CHANNEL_COUNT)
            tasa = f.getInteger(MediaFormat.KEY_SAMPLE_RATE)
          }
        }
      }
      // Cabecera WAV (PCM 16 bits, little-endian).
      val h = ByteBuffer.allocate(44).order(ByteOrder.LITTLE_ENDIAN)
      h.put("RIFF".toByteArray()); h.putInt((36 + bytes).toInt()); h.put("WAVE".toByteArray())
      h.put("fmt ".toByteArray()); h.putInt(16); h.putShort(1); h.putShort(canales.toShort())
      h.putInt(tasa); h.putInt(tasa * canales * 2); h.putShort((canales * 2).toShort()); h.putShort(16)
      h.put("data".toByteArray()); h.putInt(bytes.toInt())
      salida.seek(0); salida.write(h.array())
      return bytes > 0
    } catch (e: Exception) {
      android.util.Log.w("yappy", "decodificarAudio: $e")
      return false
    } finally {
      try { codec?.stop(); codec?.release() } catch (_: Exception) {}
      try { ex.release() } catch (_: Exception) {}
      try { salida?.close() } catch (_: Exception) {}
    }
  }

  // ─── Sonido (delegado) ──────────────────────────────────────────────────
  @JvmStatic fun nowPlaying(titulo: String?, artista: String?, album: String?, duracion: Double, posicion: Double, sonando: Boolean) =
    Sonido.nowPlaying(titulo, artista, album, duracion, posicion, sonando)
  @JvmStatic fun audioFocoPedir() = Sonido.pedirFoco()
  @JvmStatic fun lecturaViva(viva: Boolean) = Sonido.lecturaViva(viva)
  @JvmStatic fun fondoEmpezar() = Sonido.fondoEmpezar()
  @JvmStatic fun fondoTerminar() = Sonido.fondoTerminar()
  @JvmStatic fun actividadEmpezar(titulo: String, total: Int) = Sonido.actividadEmpezar(titulo, total)
  @JvmStatic fun actividadActualizar(hechas: Int, total: Int, etapa: String, titulo: String?) = Sonido.actividadActualizar(hechas, total, etapa, titulo)
  @JvmStatic fun actividadTerminar(titulo: String) = Sonido.actividadTerminar(titulo)
  @JvmStatic fun ficheroPlay(path: String, desde: Double): Boolean = Sonido.ficheroPlay(path, desde)
  @JvmStatic fun ficheroPause() = Sonido.ficheroPause()
  @JvmStatic fun ficheroResume() = Sonido.ficheroResume()
  @JvmStatic fun ficheroStop() = Sonido.ficheroStop()
  @JvmStatic fun ficheroSeek(segundos: Double) = Sonido.ficheroSeek(segundos)
  @JvmStatic fun ficheroPosicion(): Double = Sonido.ficheroPosicion()
  @JvmStatic fun ficheroDuracion(): Double = Sonido.ficheroDuracion()
  @JvmStatic fun ficheroSonando(): Boolean = Sonido.ficheroSonando()
  @JvmStatic fun ficheroActual(): String? = Sonido.ficheroActual()
  @JvmStatic fun efectoPlay(path: String): Double = Sonido.efectoPlay(path)
  @JvmStatic fun efectoStop() = Sonido.efectoStop()

  // ─── Compras (delegado) ─────────────────────────────────────────────────
  @JvmStatic fun comprasConfigurar(apiKey: String, entitlement: String) = Compras.configurar(apiKey, entitlement)
  @JvmStatic fun comprasEsPro(): Boolean = Compras.esProCache()
  @JvmStatic fun comprasOfertas(id: Long) = Compras.ofertas(id)
  @JvmStatic fun comprasComprar(id: Long, paquete: String) = Compras.comprar(id, paquete)
  @JvmStatic fun comprasRestaurar(id: Long) = Compras.restaurar(id)
  @JvmStatic fun comprasCliente(id: Long) = Compras.cliente(id)
  @JvmStatic fun comprasUsuario(): String? = Compras.usuario()
  @JvmStatic fun comprasGestionar() = Compras.gestionar()
}
