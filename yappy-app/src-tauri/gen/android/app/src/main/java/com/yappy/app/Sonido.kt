package com.yappy.app

import android.app.Notification
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.media.MediaPlayer
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import android.support.v4.media.MediaMetadataCompat
import android.support.v4.media.session.MediaSessionCompat
import android.support.v4.media.session.PlaybackStateCompat
import androidx.core.app.NotificationCompat
import androidx.media.app.NotificationCompat.MediaStyle
import java.io.File

// EL SONIDO DE LA CASA EN ANDROID. Lo que en iOS son AVAudioSession +
// MPNowPlayingInfoCenter + AVAudioPlayer + el keepalive de silencio, aquí es:
//   - una MediaSession con sus mandos (pantalla de bloqueo, auriculares,
//     Android Auto), alimentada por Rust con nowPlaying();
//   - el foco de audio (llamadas, otra app que suena → pausa; los
//     auriculares que se desconectan → pausa);
//   - UN servicio en primer plano (ServicioYappy) que mantiene el proceso
//     vivo con la pantalla apagada, con dos caras: la notificación de medios
//     mientras algo suena y la de progreso mientras la imprenta trabaja;
//   - un MediaPlayer para los audiolibros de la biblioteca y otro aparte
//     para los efectos cortos (muestras de voz, el título dicho).
object Sonido {
  private const val ID_LECTURA = 1
  private const val ID_IMPRENTA = 2

  private var sesion: MediaSessionCompat? = null
  private var foco: AudioFocusRequest? = null
  private var conFoco = false
  private var ruidoRegistrado = false
  @Volatile private var lecturaViva = false
  @Volatile private var sonandoAhora = false
  @Volatile private var imprentaViva = false
  private var wake: PowerManager.WakeLock? = null

  // Lo último que Rust dijo, para pintar la notificación de medios.
  private var titulo: String = ""
  private var duracionMs: Long = 0
  private var posicionMs: Long = 0
  // El progreso de la imprenta.
  private var impTitulo = ""
  private var impHechas = 0
  private var impTotal = 0
  private var impEtapa = "synth"

  private val app get() = Puente.app

  // ─── La sesión de medios ───────────────────────────────────────────────
  private fun sesion(): MediaSessionCompat {
    sesion?.let { return it }
    val s = MediaSessionCompat(app, "yappy")
    s.setCallback(object : MediaSessionCompat.Callback() {
      override fun onPlay() { Puente.nativoMando("play", 0.0) }
      override fun onPause() { Puente.nativoMando("pause", 0.0) }
      override fun onStop() { Puente.nativoMando("pause", 0.0) }
      // La misma semántica que dentro de la app: una FRASE, no 15 segundos.
      override fun onSkipToNext() { Puente.nativoMando("next", 0.0) }
      override fun onSkipToPrevious() { Puente.nativoMando("prev", 0.0) }
      override fun onFastForward() { Puente.nativoMando("next", 0.0) }
      override fun onRewind() { Puente.nativoMando("prev", 0.0) }
      override fun onSeekTo(pos: Long) { Puente.nativoMando("seek", pos / 1000.0) }
    })
    sesion = s
    return s
  }

  private val oyenteFoco = AudioManager.OnAudioFocusChangeListener { cambio ->
    when (cambio) {
      AudioManager.AUDIOFOCUS_LOSS -> { conFoco = false; Puente.nativoMando("interrupcion", 0.0) }
      AudioManager.AUDIOFOCUS_LOSS_TRANSIENT,
      AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> { Puente.nativoMando("interrupcion", 0.0) }
      AudioManager.AUDIOFOCUS_GAIN -> { conFoco = true; Puente.nativoMando("interrupcion", 1.0) }
    }
  }

  private val ruido = object : BroadcastReceiver() {
    override fun onReceive(c: Context?, i: Intent?) {
      if (i?.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) Puente.nativoMando("interrupcion", 0.0)
    }
  }

  /** Foco de audio para la voz hablada: las demás apps callan y luego vuelven. */
  fun pedirFoco() {
    Puente.principal.post {
      val am = app.getSystemService(Context.AUDIO_SERVICE) as AudioManager
      val atributos = AudioAttributes.Builder()
        .setUsage(AudioAttributes.USAGE_MEDIA)
        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
        .build()
      val r = if (Build.VERSION.SDK_INT >= 26) {
        val f = foco ?: AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
          .setAudioAttributes(atributos)
          .setOnAudioFocusChangeListener(oyenteFoco, Puente.principal)
          .setWillPauseWhenDucked(false)
          .build().also { foco = it }
        am.requestAudioFocus(f)
      } else @Suppress("DEPRECATION") am.requestAudioFocus(oyenteFoco, AudioManager.STREAM_MUSIC, AudioManager.AUDIOFOCUS_GAIN)
      conFoco = r == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
      if (!ruidoRegistrado) {
        app.registerReceiver(ruido, IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY))
        ruidoRegistrado = true
      }
    }
  }

  private fun soltarFoco() {
    if (!conFoco) return
    val am = app.getSystemService(Context.AUDIO_SERVICE) as AudioManager
    if (Build.VERSION.SDK_INT >= 26) foco?.let { am.abandonAudioFocusRequest(it) }
    else @Suppress("DEPRECATION") am.abandonAudioFocus(oyenteFoco)
    conFoco = false
  }

  fun lecturaViva(viva: Boolean) { lecturaViva = viva }

  /** Rust publica cada cambio: título nulo = ya no suena nada. */
  fun nowPlaying(t: String?, artista: String?, album: String?, duracion: Double, posicion: Double, sonando: Boolean) {
    Puente.principal.post {
      if (t == null || t.isEmpty()) {
        sonandoAhora = false
        sesion?.isActive = false
        sesion?.setPlaybackState(
          PlaybackStateCompat.Builder().setState(PlaybackStateCompat.STATE_STOPPED, 0, 0f).build()
        )
        if (!lecturaViva) soltarFoco()
        revisarServicio()
        return@post
      }
      titulo = t
      duracionMs = (duracion * 1000).toLong().coerceAtLeast(0)
      posicionMs = (posicion * 1000).toLong().coerceAtLeast(0)
      sonandoAhora = sonando
      val s = sesion()
      s.setMetadata(
        MediaMetadataCompat.Builder()
          .putString(MediaMetadataCompat.METADATA_KEY_TITLE, t)
          .putString(MediaMetadataCompat.METADATA_KEY_ARTIST, artista ?: "Yappy")
          .putString(MediaMetadataCompat.METADATA_KEY_ALBUM, album ?: "")
          .putLong(MediaMetadataCompat.METADATA_KEY_DURATION, duracionMs)
          .build()
      )
      s.setPlaybackState(
        PlaybackStateCompat.Builder()
          .setActions(
            PlaybackStateCompat.ACTION_PLAY or PlaybackStateCompat.ACTION_PAUSE or
              PlaybackStateCompat.ACTION_PLAY_PAUSE or PlaybackStateCompat.ACTION_SEEK_TO or
              PlaybackStateCompat.ACTION_SKIP_TO_NEXT or PlaybackStateCompat.ACTION_SKIP_TO_PREVIOUS or
              PlaybackStateCompat.ACTION_STOP
          )
          .setState(
            if (sonando) PlaybackStateCompat.STATE_PLAYING else PlaybackStateCompat.STATE_PAUSED,
            posicionMs, if (sonando) 1f else 0f
          )
          .build()
      )
      s.isActive = true
      if (sonando && !conFoco) pedirFoco()
      revisarServicio()
    }
  }

  // ─── El servicio en primer plano ───────────────────────────────────────
  private fun revisarServicio() {
    val hayLectura = sonandoAhora || (lecturaViva && titulo.isNotEmpty())
    if (hayLectura || imprentaViva) {
      tenerWake(true)
      try {
        val i = Intent(app, ServicioYappy::class.java)
        if (Build.VERSION.SDK_INT >= 26) app.startForegroundService(i) else app.startService(i)
      } catch (e: Exception) {
        // Android 14+: sin permiso para arrancar desde el fondo. La
        // sesión de medios sigue viva igual; solo falta el escudo.
        android.util.Log.w("yappy", "servicio: $e")
      }
    } else {
      tenerWake(false)
      app.stopService(Intent(app, ServicioYappy::class.java))
    }
  }

  private fun tenerWake(si: Boolean) {
    if (si) {
      if (wake?.isHeld == true) return
      val pm = app.getSystemService(Context.POWER_SERVICE) as PowerManager
      wake = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "yappy:voz").also { it.setReferenceCounted(false); it.acquire() }
    } else {
      wake?.let { if (it.isHeld) it.release() }
      wake = null
    }
  }

  fun abrirApp(): PendingIntent = PendingIntent.getActivity(
    app, 0, Intent(app, MainActivity::class.java).apply { flags = Intent.FLAG_ACTIVITY_SINGLE_TOP },
    PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
  )

  private fun accion(icono: Int, texto: String, accion: Long): NotificationCompat.Action =
    NotificationCompat.Action(
      icono, texto,
      androidx.media.session.MediaButtonReceiver.buildMediaButtonPendingIntent(app, accion)
    )

  fun notificacionLectura(): Notification {
    val b = NotificationCompat.Builder(app, Puente.CANAL_LECTURA)
      .setSmallIcon(R.drawable.ic_yappy_aviso)
      .setContentTitle(titulo.ifEmpty { "Yappy" })
      .setContentText("Yappy")
      .setContentIntent(abrirApp())
      .setOnlyAlertOnce(true)
      .setOngoing(sonandoAhora)
      .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
      .setPriority(NotificationCompat.PRIORITY_LOW)
      .addAction(accion(android.R.drawable.ic_media_previous, "Anterior", PlaybackStateCompat.ACTION_SKIP_TO_PREVIOUS))
      .addAction(
        if (sonandoAhora) accion(android.R.drawable.ic_media_pause, "Pausa", PlaybackStateCompat.ACTION_PAUSE)
        else accion(android.R.drawable.ic_media_play, "Seguir", PlaybackStateCompat.ACTION_PLAY)
      )
      .addAction(accion(android.R.drawable.ic_media_next, "Siguiente", PlaybackStateCompat.ACTION_SKIP_TO_NEXT))
    sesion?.let {
      b.setStyle(MediaStyle().setMediaSession(it.sessionToken).setShowActionsInCompactView(0, 1, 2))
    }
    return b.build()
  }

  fun notificacionImprenta(): Notification {
    val etapa = if (impEtapa == "writing") "Encuadernando" else "Sintetizando"
    val b = NotificationCompat.Builder(app, Puente.CANAL_IMPRENTA)
      .setSmallIcon(R.drawable.ic_yappy_aviso)
      .setContentTitle(impTitulo.ifEmpty { "Audiolibro en marcha" })
      .setContentText(if (impTotal > 0) "$etapa · $impHechas de $impTotal" else etapa)
      .setContentIntent(abrirApp())
      .setOnlyAlertOnce(true)
      .setOngoing(true)
      .setPriority(NotificationCompat.PRIORITY_LOW)
    if (impTotal > 0) b.setProgress(impTotal, impHechas.coerceIn(0, impTotal), false)
    else b.setProgress(0, 0, true)
    return b.build()
  }

  fun estado(): Triple<Boolean, Boolean, Boolean> = Triple(sonandoAhora || (lecturaViva && titulo.isNotEmpty()), imprentaViva, sonandoAhora)

  // ─── La imprenta: escudo de fondo + progreso (la Live Activity de aquí) ─
  fun fondoEmpezar() { Puente.principal.post { imprentaViva = true; revisarServicio() } }
  fun fondoTerminar() { Puente.principal.post { imprentaViva = false; revisarServicio() } }
  fun actividadEmpezar(t: String, total: Int) {
    Puente.principal.post { impTitulo = t; impTotal = total; impHechas = 0; impEtapa = "synth"; imprentaViva = true; revisarServicio() }
  }
  fun actividadActualizar(hechas: Int, total: Int, etapa: String, t: String?) {
    Puente.principal.post {
      impHechas = hechas; impTotal = total; impEtapa = etapa; if (t != null) impTitulo = t
      if (imprentaViva) ServicioYappy.actual?.repintar()
    }
  }
  fun actividadTerminar(t: String) { Puente.principal.post { imprentaViva = false; revisarServicio() } }

  // ─── El reproductor de ficheros (la biblioteca) ────────────────────────
  private var player: MediaPlayer? = null
  @Volatile private var rutaActual: String? = null
  @Volatile private var preparado = false

  fun ficheroPlay(path: String, desde: Double): Boolean {
    if (rutaActual == path && player != null && preparado) {
      player?.start(); return true
    }
    ficheroStop()
    if (!File(path).exists()) return false
    return try {
      val p = MediaPlayer()
      p.setAudioAttributes(
        AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).setContentType(AudioAttributes.CONTENT_TYPE_SPEECH).build()
      )
      p.setDataSource(path)
      p.setOnCompletionListener { rutaActual = null; preparado = false }
      p.prepare()
      preparado = true
      if (desde > 0 && desde * 1000 < p.duration) p.seekTo((desde * 1000).toInt())
      p.start()
      player = p
      rutaActual = path
      pedirFoco()
      true
    } catch (e: Exception) {
      android.util.Log.w("yappy", "ficheroPlay: $e"); false
    }
  }
  fun ficheroPause() { try { player?.pause() } catch (_: Exception) {} }
  fun ficheroResume() { try { player?.start() } catch (_: Exception) {} }
  fun ficheroStop() {
    try { player?.stop(); player?.release() } catch (_: Exception) {}
    player = null; rutaActual = null; preparado = false
  }
  fun ficheroSeek(segundos: Double) {
    val p = player ?: return
    try {
      val ms = (segundos * 1000).toLong().coerceIn(0, p.duration.toLong())
      if (Build.VERSION.SDK_INT >= 26) p.seekTo(ms, MediaPlayer.SEEK_CLOSEST) else p.seekTo(ms.toInt())
    } catch (_: Exception) {}
  }
  fun ficheroPosicion(): Double = try { if (preparado) (player?.currentPosition ?: 0) / 1000.0 else 0.0 } catch (_: Exception) { 0.0 }
  fun ficheroDuracion(): Double = try { if (preparado) (player?.duration ?: 0) / 1000.0 else 0.0 } catch (_: Exception) { 0.0 }
  fun ficheroSonando(): Boolean = try { player?.isPlaying == true } catch (_: Exception) { false }
  fun ficheroActual(): String? = rutaActual

  // ─── El canal de efectos ───────────────────────────────────────────────
  private var efecto: MediaPlayer? = null
  fun efectoPlay(path: String): Double {
    if (!File(path).exists()) return 0.0
    return try {
      efectoStop()
      val p = MediaPlayer()
      p.setAudioAttributes(
        AudioAttributes.Builder().setUsage(AudioAttributes.USAGE_MEDIA).setContentType(AudioAttributes.CONTENT_TYPE_SPEECH).build()
      )
      p.setDataSource(path)
      p.prepare()
      p.setOnCompletionListener { it.release(); if (efecto === it) efecto = null }
      p.start()
      efecto = p
      p.duration / 1000.0
    } catch (e: Exception) {
      android.util.Log.w("yappy", "efectoPlay: $e"); 0.0
    }
  }
  fun efectoStop() {
    try { efecto?.stop(); efecto?.release() } catch (_: Exception) {}
    efecto = null
  }
}

/** El escudo: el proceso sigue vivo con la pantalla apagada mientras algo
 *  suena o la imprenta trabaja. Dos caras de notificación según el caso. */
class ServicioYappy : Service() {
  companion object { var actual: ServicioYappy? = null }

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onCreate() { super.onCreate(); actual = this }
  override fun onDestroy() { actual = null; super.onDestroy() }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    if (intent != null) androidx.media.session.MediaButtonReceiver.handleIntent(sesionDeSonido(), intent)
    repintar()
    return START_NOT_STICKY
  }

  private fun sesionDeSonido(): MediaSessionCompat? {
    // Reflexión mínima sobre el objeto: la sesión es privada en Sonido.
    return try {
      val f = Sonido::class.java.getDeclaredField("sesion"); f.isAccessible = true; f.get(Sonido) as? MediaSessionCompat
    } catch (_: Exception) { null }
  }

  fun repintar() {
    val (hayLectura, hayImprenta, _) = Sonido.estado()
    try {
      when {
        hayLectura -> primerPlano(1, Sonido.notificacionLectura(), ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
        hayImprenta -> primerPlano(
          2, Sonido.notificacionImprenta(),
          if (Build.VERSION.SDK_INT >= 35) ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROCESSING
          else ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC
        )
        else -> { stopForeground(STOP_FOREGROUND_REMOVE); stopSelf() }
      }
    } catch (e: Exception) {
      android.util.Log.w("yappy", "primer plano: $e")
    }
  }

  private fun primerPlano(id: Int, n: Notification, tipo: Int) {
    if (Build.VERSION.SDK_INT >= 29) startForeground(id, n, tipo) else startForeground(id, n)
  }
}
