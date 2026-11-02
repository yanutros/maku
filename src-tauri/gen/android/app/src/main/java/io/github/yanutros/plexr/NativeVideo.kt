package io.github.yanutros.plexr

import android.app.Activity
import android.graphics.Color
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.view.Gravity
import android.view.SurfaceView
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.webkit.WebView
import android.widget.FrameLayout
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.Tracks
import androidx.media3.common.VideoSize
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.ui.SubtitleView
import org.json.JSONArray
import org.json.JSONObject

/**
 * Lecteur vidéo natif (ExoPlayer), comme Plex et Wholphin : le fichier d'origine est décodé par la
 * puce de l'appareil et affiché dans sa propre couche (SurfaceView), sous la page devenue
 * transparente — l'interface de lecture de Maku reste dessinée par-dessus. La page ne redessine
 * donc plus chaque image de la vidéo (c'était la cause des saccades sur téléviseur).
 *
 * La page pilote le lecteur (`PlexrDevice.video*`) et reçoit son état par `window.__plexrVideo`.
 * Les pistes sont numérotées comme dans mpv : à partir de 1, dans l'ordre, pour chaque type.
 */
class NativeVideo(private val activity: Activity, private val webView: WebView) {
  private val handler = Handler(Looper.getMainLooper())
  private var player: ExoPlayer? = null
  private var surface: SurfaceView? = null
  /** Noir profond derrière l'image, visible dans les bandes (format plus large que l'écran). */
  private var black: View? = null
  private var subtitles: SubtitleView? = null
  /** Pistes demandées par la page, appliquées dès que le fichier est analysé. */
  private var wantedAudio: Int? = null
  private var wantedSub: String = "auto"
  private var tracksApplied = false
  /** Les pistes du fichier ne sont pas lisibles ici : la page bascule sur la conversion du serveur. */
  private var unsupportedSent = false

  // ---------- Commandes de la page (appelées depuis le fil du WebView) ----------

  /** JSON : url, offsetMs, aid, sid (« no », « auto » ou numéro), subFiles [{url, codec, language, title}]. */
  fun open(json: String) = handler.post {
    val o = JSONObject(json)
    close()
    wantedAudio = if (o.isNull("aid")) null else o.optInt("aid")
    wantedSub = o.optString("sid", "auto")
    tracksApplied = false
    unsupportedSent = false

    val root = webView.parent as ViewGroup
    val sv = SurfaceView(activity)
    val st = SubtitleView(activity).apply {
      setUserDefaultStyle()
      setUserDefaultTextSize()
    }
    val bg = View(activity).apply { setBackgroundColor(Color.BLACK) }
    // Sous la page : un fond noir, la vidéo, puis ses sous-titres.
    root.addView(bg, 0, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
    root.addView(sv, 1, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
    root.addView(st, 2, FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT))
    black = bg
    surface = sv
    subtitles = st
    webView.setBackgroundColor(Color.TRANSPARENT)
    activity.window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)

    val subs = o.optJSONArray("subFiles") ?: JSONArray()
    val item = MediaItem.Builder()
      .setUri(o.getString("url"))
      .setSubtitleConfigurations(
        (0 until subs.length()).map { i ->
          val s = subs.getJSONObject(i)
          MediaItem.SubtitleConfiguration.Builder(Uri.parse(s.getString("url")))
            .setMimeType(subtitleMime(s.optString("codec")))
            .setLanguage(s.optString("language").ifEmpty { null })
            .setLabel(s.optString("title").ifEmpty { null })
            .build()
        },
      )
      .build()

    // Les bandes-annonces de plex.tv sont servies par le serveur en http puis redirigées en https :
    // ExoPlayer refuse ce changement de protocole par défaut.
    val http = DefaultHttpDataSource.Factory().setAllowCrossProtocolRedirects(true)
    val p = ExoPlayer.Builder(activity, DefaultRenderersFactory(activity).setEnableDecoderFallback(true))
      .setMediaSourceFactory(DefaultMediaSourceFactory(DefaultDataSource.Factory(activity, http)))
      .setAudioAttributes(AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MOVIE).build(), true)
      .setVideoChangeFrameRateStrategy(C.VIDEO_CHANGE_FRAME_RATE_STRATEGY_ONLY_IF_SEAMLESS)
      .build()
    p.setVideoSurfaceView(sv)
    p.addListener(object : Player.Listener {
      override fun onTracksChanged(tracks: Tracks) {
        applyWantedTracks(p, tracks)
        checkSupport(tracks)
        send()
      }
      override fun onCues(cueGroup: androidx.media3.common.text.CueGroup) {
        subtitles?.setCues(cueGroup.cues)
      }
      override fun onVideoSizeChanged(videoSize: VideoSize) = fit(videoSize)
      override fun onIsPlayingChanged(isPlaying: Boolean) = send()
      override fun onPlaybackStateChanged(state: Int) {
        if (state == Player.STATE_ENDED) emit(JSONObject().put("ended", true)) else send()
      }
      override fun onPlayerError(error: PlaybackException) {
        // Format ou codec que l'appareil ne sait pas lire : la page demande alors la conversion.
        val decoding = error.errorCode in 4000..4999 || error.errorCode == PlaybackException.ERROR_CODE_PARSING_CONTAINER_UNSUPPORTED
        emit(JSONObject().put(if (decoding) "unsupported" else "error", error.message ?: error.errorCodeName))
      }
    })
    p.setMediaItem(item, o.optLong("offsetMs"))
    p.prepare()
    p.play()
    player = p
    tick()
  }

  /** Commandes au format mpv : pause, play, toggle, seek (ms, absolu), seekBy (ms), volume (0-100), mute, aid / sid (numéro ou « no »). */
  fun command(name: String, value: String) = handler.post {
    val p = player ?: return@post
    when (name) {
      "toggle" -> p.playWhenReady = !p.playWhenReady
      "pause" -> p.playWhenReady = false
      "play" -> p.playWhenReady = true
      "seek" -> p.seekTo(value.toLong().coerceAtLeast(0))
      "seekBy" -> p.seekTo((p.currentPosition + value.toLong()).coerceAtLeast(0))
      "volume" -> p.volume = (value.toFloat() / 100f).coerceIn(0f, 1f)
      "mute" -> p.volume = if (value == "1") 0f else 1f
      "aid" -> select(p, C.TRACK_TYPE_AUDIO, value)
      "sid" -> select(p, C.TRACK_TYPE_TEXT, value)
    }
    send()
  }

  /**
   * Garde le format de l'image (bandes noires en haut et en bas pour un film en 2.39:1, comme dans
   * Plex) : la couche vidéo prend la plus grande taille qui tient dans l'écran, centrée.
   */
  private fun fit(size: VideoSize) {
    val sv = surface ?: return
    val root = sv.parent as? ViewGroup ?: return
    if (size.width == 0 || size.height == 0 || root.width == 0 || root.height == 0) return
    val ratio = size.width * size.pixelWidthHeightRatio / size.height
    val (w, h) = if (root.width.toFloat() / root.height > ratio) {
      (root.height * ratio).toInt() to root.height
    } else {
      root.width to (root.width / ratio).toInt()
    }
    sv.layoutParams = FrameLayout.LayoutParams(w, h, Gravity.CENTER)
  }

  fun close() {
    handler.removeCallbacksAndMessages(null)
    player?.release()
    player = null
    val root = webView.parent as? ViewGroup
    surface?.let { root?.removeView(it) }
    subtitles?.let { root?.removeView(it) }
    black?.let { root?.removeView(it) }
    black = null
    surface = null
    subtitles = null
    activity.window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
  }

  fun closeAsync() = handler.post { close() }

  // ---------- Pistes ----------

  /** Groupes d'un type, dans l'ordre du fichier (sous-titres externes à la fin). */
  private fun groups(tracks: Tracks, type: Int) = tracks.groups.filter { it.type == type }

  private fun applyWantedTracks(p: ExoPlayer, tracks: Tracks) {
    if (tracksApplied || tracks.groups.isEmpty()) return
    tracksApplied = true
    wantedAudio?.let { select(p, C.TRACK_TYPE_AUDIO, it.toString()) }
    when (wantedSub) {
      "auto" -> Unit
      else -> select(p, C.TRACK_TYPE_TEXT, wantedSub)
    }
  }

  private fun select(p: ExoPlayer, type: Int, value: String) {
    val params = p.trackSelectionParameters.buildUpon()
    if (value == "no") {
      params.setTrackTypeDisabled(type, true)
    } else {
      val group = groups(p.currentTracks, type).getOrNull((value.toIntOrNull() ?: return) - 1) ?: return
      params.setTrackTypeDisabled(type, false).setOverrideForType(TrackSelectionOverride(group.mediaTrackGroup, 0))
    }
    p.trackSelectionParameters = params.build()
  }

  /** Aucune piste vidéo (ou audio) décodable : on prévient la page une fois. */
  private fun checkSupport(tracks: Tracks) {
    if (unsupportedSent) return
    for (type in listOf(C.TRACK_TYPE_VIDEO, C.TRACK_TYPE_AUDIO)) {
      val gs = groups(tracks, type)
      if (gs.isNotEmpty() && gs.none { it.isSupported }) {
        unsupportedSent = true
        emit(JSONObject().put("unsupported", if (type == C.TRACK_TYPE_VIDEO) "video" else "audio"))
        return
      }
    }
  }

  // ---------- État vers la page ----------

  private fun tick() {
    send()
    handler.postDelayed({ if (player != null) tick() }, 500)
  }

  private fun send() {
    val p = player ?: return
    val tracks = JSONArray()
    var aid: Int? = null
    var sid: Int? = null
    for ((type, name) in listOf(C.TRACK_TYPE_AUDIO to "audio", C.TRACK_TYPE_TEXT to "sub")) {
      groups(p.currentTracks, type).forEachIndexed { i, g ->
        val f = g.getTrackFormat(0)
        val t = JSONObject().put("id", i + 1).put("type", name)
        f.label?.let { t.put("title", it) }
        f.language?.takeIf { it != C.LANGUAGE_UNDETERMINED }?.let { t.put("lang", it) }
        f.codecs?.let { t.put("codec", it) }
        if (f.channelCount > 0) t.put("demux-channel-count", f.channelCount)
        if (f.selectionFlags and C.SELECTION_FLAG_FORCED != 0) t.put("forced", true)
        if (!g.isSupported) t.put("unsupported", true)
        tracks.put(t)
        if (g.isSelected) if (type == C.TRACK_TYPE_AUDIO) aid = i + 1 else sid = i + 1
      }
    }
    emit(
      JSONObject()
        .put("position", p.currentPosition)
        .put("duration", if (p.duration > 0) p.duration else 0)
        .put("buffered", p.bufferedPosition)
        .put("paused", !p.playWhenReady)
        .put("buffering", p.playbackState == Player.STATE_BUFFERING || p.playbackState == Player.STATE_IDLE)
        .put("volume", (p.volume * 100).toInt())
        .put("tracks", tracks)
        .put("aid", aid ?: JSONObject.NULL)
        .put("sid", sid ?: JSONObject.NULL),
    )
  }

  private fun emit(o: JSONObject) {
    webView.evaluateJavascript("window.__plexrVideo && window.__plexrVideo($o)", null)
  }

  private fun subtitleMime(codec: String) = when (codec.lowercase()) {
    "ass", "ssa" -> MimeTypes.TEXT_SSA
    "vtt", "webvtt" -> MimeTypes.TEXT_VTT
    "ttml", "dfxp" -> MimeTypes.APPLICATION_TTML
    else -> MimeTypes.APPLICATION_SUBRIP
  }
}
