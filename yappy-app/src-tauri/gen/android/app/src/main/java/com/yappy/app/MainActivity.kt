package com.yappy.app

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import java.io.File

// La puerta de entrada Android: los intents de compartir (texto, enlace,
// PDF, EPUB, Word, audio) se escriben como líneas en files/yappy-shared.txt
// con el mismo protocolo que el App Group de iOS (url:/text:/file:/audio:),
// y el frontend los drena al hacerse visible. Los ficheros compartidos se
// copian a files/compartidos/ porque el content:// del emisor caduca.
class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    intent?.let { atenderIntent(it) }
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    atenderIntent(intent)
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
            texto != null && (texto.startsWith("http://") || texto.startsWith("https://")) ->
              encolar("url:$texto")
            texto != null -> encolar("text:$texto")
          }
        }
        Intent.ACTION_VIEW -> {
          // «Abrir con Yappy» desde un gestor de archivos.
          intent.data?.let { uri ->
            if (uri.scheme == "content" || uri.scheme == "file") {
              encolarFichero(uri, intent.type)
            }
          }
        }
      }
    } catch (e: Exception) {
      android.util.Log.w("yappy", "atenderIntent: $e")
    }
  }

  private fun encolarFichero(uri: Uri, tipo: String?) {
    val dir = File(filesDir, "compartidos").apply { mkdirs() }
    val nombre = consultarNombre(uri) ?: "compartido"
    val destino = File(dir, "${System.currentTimeMillis()}-$nombre")
    contentResolver.openInputStream(uri)?.use { entrada ->
      destino.outputStream().use { salida -> entrada.copyTo(salida) }
    } ?: return
    val prefijo = if (tipo?.startsWith("audio/") == true) "audio" else "file"
    encolar("$prefijo:${destino.absolutePath}")
  }

  private fun consultarNombre(uri: Uri): String? =
    contentResolver.query(uri, null, null, null, null)?.use { c ->
      val i = c.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
      if (i >= 0 && c.moveToFirst()) c.getString(i)?.replace('/', '-') else null
    }

  private fun encolar(linea: String) {
    val f = File(filesDir, "yappy-shared.txt")
    f.appendText(linea + "\n")
    android.util.Log.i("yappy", "compartido encolado: ${linea.take(48)}")
  }
}
