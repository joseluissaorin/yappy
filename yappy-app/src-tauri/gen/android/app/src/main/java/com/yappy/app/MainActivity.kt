package com.yappy.app

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import java.io.File

// La puerta de entrada Android: los intents de compartir (texto, enlace,
// PDF, EPUB, Word, audio, .yappy) se escriben como líneas en
// files/yappy-shared.txt con el mismo protocolo que el App Group de iOS
// (url:/text:/file:/audio:), y el frontend los drena al hacerse visible. Los
// ficheros compartidos se copian a files/compartidos/ porque el content://
// del emisor caduca. Además arranca el Puente (avisos, sonido, tienda).
class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    Puente.arrancar(this)
    intent?.let { atenderIntent(it) }
  }

  override fun onResume() {
    super.onResume()
    Puente.actividad = this
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    atenderIntent(intent)
  }

  override fun onRequestPermissionsResult(requestCode: Int, permissions: Array<out String>, grantResults: IntArray) {
    super.onRequestPermissionsResult(requestCode, permissions, grantResults)
    if (requestCode == 7101) Puente.avisosRespondido()
  }

  private fun atenderIntent(intent: Intent) {
    try {
      when (intent.action) {
        Intent.ACTION_SEND -> {
          val texto = intent.getStringExtra(Intent.EXTRA_TEXT)
          val stream: Uri? =
            if (android.os.Build.VERSION.SDK_INT >= 33)
              intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
            else @Suppress("DEPRECATION") intent.getParcelableExtra(Intent.EXTRA_STREAM)
          when {
            stream != null -> encolarFichero(stream, intent.type)
            texto != null && esEnlace(texto) -> encolar("url:${texto.trim()}")
            texto != null -> encolar("text:$texto")
          }
        }
        Intent.ACTION_SEND_MULTIPLE -> {
          val streams: List<Uri> =
            if (android.os.Build.VERSION.SDK_INT >= 33)
              intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java) ?: emptyList()
            else @Suppress("DEPRECATION") intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM) ?: emptyList()
          streams.forEach { encolarFichero(it, intent.type) }
        }
        Intent.ACTION_VIEW -> {
          // «Abrir con Yappy» desde un gestor de archivos o el navegador.
          intent.data?.let { uri ->
            when (uri.scheme) {
              "content", "file" -> encolarFichero(uri, intent.type)
              "http", "https" -> encolar("url:$uri")
            }
          }
        }
      }
    } catch (e: Exception) {
      android.util.Log.w("yappy", "atenderIntent: $e")
    }
  }

  // Chrome comparte «Título\nhttps://…» a veces: el enlace manda.
  private fun esEnlace(t: String): Boolean {
    val s = t.trim()
    return s.startsWith("http://") || s.startsWith("https://")
  }

  private fun encolarFichero(uri: Uri, tipo: String?) {
    val dir = File(filesDir, "compartidos").apply { mkdirs() }
    val nombre = consultarNombre(uri) ?: "compartido"
    val destino = File(dir, "${System.currentTimeMillis()}-$nombre")
    contentResolver.openInputStream(uri)?.use { entrada ->
      destino.outputStream().use { salida -> entrada.copyTo(salida) }
    } ?: return
    val esAudio = tipo?.startsWith("audio/") == true ||
      nombre.substringAfterLast('.', "").lowercase() in setOf("m4a", "mp3", "wav", "ogg", "opus", "aac", "flac", "oga", "amr", "3gp")
    val prefijo = if (esAudio) "audio" else "file"
    encolar("$prefijo:${destino.absolutePath}")
  }

  private fun consultarNombre(uri: Uri): String? {
    if (uri.scheme == "file") return uri.lastPathSegment?.replace('/', '-')
    return contentResolver.query(uri, null, null, null, null)?.use { c ->
      val i = c.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
      if (i >= 0 && c.moveToFirst()) c.getString(i)?.replace('/', '-') else null
    }
  }

  private fun encolar(linea: String) {
    val f = File(filesDir, "yappy-shared.txt")
    f.appendText(linea + "\n")
    android.util.Log.i("yappy", "compartido encolado: ${linea.take(48)}")
  }
}
