package io.github.yanutros.plexr

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.Process
import android.util.Log
import android.support.v4.media.MediaBrowserCompat
import android.support.v4.media.MediaDescriptionCompat
import android.support.v4.media.MediaMetadataCompat
import android.support.v4.media.session.MediaSessionCompat
import android.support.v4.media.session.PlaybackStateCompat
import android.webkit.WebView
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import androidx.media.MediaBrowserServiceCompat
import androidx.media.app.NotificationCompat.MediaStyle
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import org.json.JSONObject
import java.net.URL
import kotlin.concurrent.thread

/**
 * Musique hors de la page. Deux sources :
 * - la page (élément <audio>) : ce service la garde en vie quand Maku est quitté et l'expose à
 *   Android par une session média — notification avec pochette et commandes, écran de verrouillage,
 *   casque Bluetooth, écran de la voiture. Les commandes repartent vers la page (`window.__plexrMedia`) ;
 * - Android Auto : la voiture parcourt la bibliothèque musicale (PlexLibrary) et ce qu'elle lance
 *   est lu ici, par ExoPlayer, même si Maku n'est pas ouvert. La page s'arrête alors, et
 *   inversement une musique lancée depuis la page reprend la main.
 */
class MediaService : MediaBrowserServiceCompat() {
  companion object {
    private const val CHANNEL = "playback"
    private const val NOTIFICATION_ID = 1

    /** Applications autorisées à parcourir la bibliothèque : Android Auto, l'Assistant, le système. */
    private val BROWSERS = PlexLibrary.ART_READERS + listOf("com.google.android.googlequicksearchbox", "com.google.android.carassistant")

    var instance: MediaService? = null
      private set

    /** Page qui reçoit les commandes (posée par MainActivity). */
    var webView: WebView? = null

    /** La page joue une musique (et non en pause). */
    val playing: Boolean
      get() = instance?.let { it.player == null && it.state.optBoolean("playing") } == true

    /** État envoyé par la page (JSON : title, artist, album, art, playing, position, duration). */
    fun update(context: Context, json: String) {
      val service = instance
      if (service == null) {
        ContextCompat.startForegroundService(context, Intent(context, MediaService::class.java).putExtra("state", json))
      } else {
        service.handler.post { service.fromPage(JSONObject(json)) }
      }
    }

    /** La page n'a plus rien à jouer (sans effet si la voiture a lancé sa propre lecture). */
    fun stop() {
      instance?.let { it.handler.post { if (it.player == null) it.stopPlayback() } }
    }

    /** Commande vers la page : play, pause, next, previous, stop, seek:<ms>. */
    fun dispatch(action: String) {
      webView?.post { webView?.evaluateJavascript("window.__plexrMedia && window.__plexrMedia('$action')", null) }
    }
  }

  private val handler = Handler(Looper.getMainLooper())
  private val library by lazy { PlexLibrary(this) }
  private lateinit var session: MediaSessionCompat
  private var state = JSONObject()
  private var artUrl: String? = null
  private var art: Bitmap? = null

  // Lecture lancée depuis la voiture.
  private var player: ExoPlayer? = null
  private var queue: List<PlexLibrary.Track> = emptyList()
  private var queueArt: List<String> = emptyList()
  private var lastTimeline = 0L
  private var activeItem = MediaSessionCompat.QueueItem.UNKNOWN_ID.toLong()

  override fun onCreate() {
    super.onCreate()
    instance = this
    session = MediaSessionCompat(this, "Maku").apply {
      setCallback(object : MediaSessionCompat.Callback() {
        override fun onPlay() = command("play")
        override fun onPause() = command("pause")
        override fun onSkipToNext() = command("next")
        override fun onSkipToPrevious() = command("previous")
        override fun onStop() = command("stop")
        override fun onSeekTo(pos: Long) = command("seek:$pos")
        override fun onSkipToQueueItem(id: Long) {
          player?.seekTo(id.toInt(), 0)
        }
        override fun onPlayFromMediaId(mediaId: String, extras: Bundle?) = playNative { library.queue(mediaId) }
        override fun onPlayFromSearch(query: String?, extras: Bundle?) = playNative { library.search(query.orEmpty()) }
      })
      setSessionActivity(
        PendingIntent.getActivity(
          this@MediaService, 0,
          Intent(this@MediaService, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
          PendingIntent.FLAG_IMMUTABLE,
        ),
      )
      isActive = true
    }
    sessionToken = session.sessionToken
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      getSystemService(NotificationManager::class.java).createNotificationChannel(
        NotificationChannel(CHANNEL, getString(R.string.app_name), NotificationManager.IMPORTANCE_LOW),
      )
    }
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    // Boutons de la notification.
    intent?.action?.let { command(it) }
    intent?.getStringExtra("state")?.let { fromPage(JSONObject(it)) }
    return START_NOT_STICKY
  }

  /** Commande de la notification, de la voiture ou d'un casque : vers la lecture en cours. */
  private fun command(action: String) {
    val p = player ?: return dispatch(action)
    when {
      action == "play" -> p.play()
      action == "pause" -> p.pause()
      action == "next" -> p.seekToNextMediaItem()
      action == "previous" -> if (p.currentPosition > 3000) p.seekTo(0) else p.seekToPreviousMediaItem()
      action == "stop" -> stopPlayback()
      action.startsWith("seek:") -> p.seekTo(action.removePrefix("seek:").toLong())
    }
  }

  private fun fromPage(next: JSONObject) {
    if (player != null) {
      // Une musique lancée depuis la page remplace celle de la voiture.
      if (!next.optBoolean("playing")) return
      stopNative()
    }
    applyState(next)
  }

  // ---------- Android Auto : bibliothèque ----------

  override fun onGetRoot(clientPackageName: String, clientUid: Int, rootHints: Bundle?): BrowserRoot {
    val trusted = clientPackageName == packageName || clientPackageName in BROWSERS || clientUid == Process.SYSTEM_UID
    return BrowserRoot(
      if (trusted) "root" else "empty",
      Bundle().apply {
        putInt("android.media.browse.CONTENT_STYLE_BROWSABLE_HINT", 1)
        putInt("android.media.browse.CONTENT_STYLE_PLAYABLE_HINT", 1)
      },
    )
  }

  override fun onLoadChildren(parentId: String, result: Result<MutableList<MediaBrowserCompat.MediaItem>>) {
    if (parentId == "empty") return result.sendResult(mutableListOf())
    result.detach()
    thread {
      val items = runCatching { library.children(parentId) }.getOrElse {
        handler.post { showError(it) }
        emptyList()
      }
      result.sendResult(items.toMutableList())
    }
  }

  // ---------- Android Auto : lecture ----------

  private fun playNative(load: () -> Pair<List<PlexLibrary.Track>, Int>) {
    thread {
      val result = runCatching {
        val (tracks, start) = load()
        Triple(tracks, start, tracks.map { library.streamUrl(it) to (it.thumb?.let(library::artUrl) ?: "") })
      }
      handler.post {
        result.onSuccess { (tracks, start, urls) ->
          if (tracks.isEmpty()) showError(null) else startNative(tracks, start, urls.map { it.first }, urls.map { it.second })
        }.onFailure(::showError)
      }
    }
  }

  private fun startNative(tracks: List<PlexLibrary.Track>, start: Int, urls: List<String>, arts: List<String>) {
    dispatch("stop")
    val p = player ?: ExoPlayer.Builder(this)
      .setAudioAttributes(AudioAttributes.Builder().setUsage(C.USAGE_MEDIA).setContentType(C.AUDIO_CONTENT_TYPE_MUSIC).build(), true)
      .setHandleAudioBecomingNoisy(true)
      .build()
      .also {
        it.addListener(object : Player.Listener {
          override fun onEvents(player: Player, events: Player.Events) = nativeChanged()
          override fun onMediaItemTransition(item: androidx.media3.common.MediaItem?, reason: Int) {
            lastTimeline = 0
          }
          override fun onPlayerError(error: PlaybackException) {
            showError(error)
            stopNative()
          }
          override fun onPlaybackStateChanged(playbackState: Int) {
            if (playbackState == Player.STATE_ENDED) stopPlayback()
          }
        })
        player = it
      }
    queue = tracks
    queueArt = arts
    p.setMediaItems(urls.map { androidx.media3.common.MediaItem.fromUri(it) }, start, 0)
    p.prepare()
    p.play()
    // Lancée depuis la voiture, la lecture doit survivre à la fin de sa connexion au service.
    runCatching { ContextCompat.startForegroundService(this, Intent(this, MediaService::class.java)) }
    session.setQueue(
      tracks.mapIndexed { i, t ->
        MediaSessionCompat.QueueItem(MediaDescriptionCompat.Builder().setMediaId("q$i").setTitle(t.title).setSubtitle(t.artist).build(), i.toLong())
      },
    )
    handler.removeCallbacks(tick)
    handler.post(tick)
  }

  /** Position à jour toutes les 10 s, et progression envoyée à Plex. */
  private val tick: Runnable = object : Runnable {
    override fun run() {
      if (player == null) return
      nativeChanged()
      handler.postDelayed(this, 10_000)
    }
  }

  private fun nativeChanged() {
    val p = player ?: return
    val track = queue.getOrNull(p.currentMediaItemIndex) ?: return
    val playing = p.playWhenReady && p.playbackState != Player.STATE_ENDED
    activeItem = p.currentMediaItemIndex.toLong()
    applyState(
      JSONObject()
        .put("title", track.title)
        .put("artist", track.artist)
        .put("album", track.album)
        .put("art", queueArt.getOrElse(p.currentMediaItemIndex) { "" })
        .put("playing", playing)
        .put("position", p.currentPosition)
        .put("duration", if (p.duration > 0) p.duration else track.durationMs),
    )
    val now = System.currentTimeMillis()
    if (now - lastTimeline > 9_000) {
      lastTimeline = now
      val position = p.currentPosition
      thread { library.timeline(track, if (playing) "playing" else "paused", position) }
    }
  }

  private fun stopNative() {
    val p = player ?: return
    queue.getOrNull(p.currentMediaItemIndex)?.let { t ->
      val position = p.currentPosition
      thread { library.timeline(t, "stopped", position) }
    }
    player = null
    p.release()
    queue = emptyList()
    handler.removeCallbacks(tick)
    session.setQueue(null)
    activeItem = MediaSessionCompat.QueueItem.UNKNOWN_ID.toLong()
  }

  /** Plus rien à jouer : la notification disparaît (le service reste lié à la voiture si besoin). */
  private fun stopPlayback() {
    stopNative()
    state = JSONObject()
    session.setPlaybackState(PlaybackStateCompat.Builder().setActions(ACTIONS).setState(PlaybackStateCompat.STATE_STOPPED, 0, 0f).build())
    ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
    stopSelf()
  }

  /** Erreur affichée sur l'écran de la voiture. */
  private fun showError(error: Throwable?) {
    Log.w("Maku", "Android Auto", error)
    val (code, message) = when (error) {
      null -> PlaybackStateCompat.ERROR_CODE_NOT_SUPPORTED to R.string.auto_nothing
      // Selon la cause : serveur à choisir, téléphone jamais déverrouillé depuis l'allumage, ou compte.
      is PlexLibrary.NotSignedIn -> PlaybackStateCompat.ERROR_CODE_AUTHENTICATION_EXPIRED to when (error.reason) {
        "server" -> R.string.auto_no_server
        "config" -> R.string.auto_locked
        else -> R.string.auto_sign_in
      }
      else -> {
        library.forgetServer()
        PlaybackStateCompat.ERROR_CODE_APP_ERROR to R.string.auto_unreachable
      }
    }
    session.setPlaybackState(
      PlaybackStateCompat.Builder().setActions(ACTIONS).setState(PlaybackStateCompat.STATE_ERROR, 0, 0f).setErrorMessage(code, getString(message)).build(),
    )
  }

  // ---------- Session et notification ----------

  private val ACTIONS = PlaybackStateCompat.ACTION_PLAY or PlaybackStateCompat.ACTION_PAUSE or PlaybackStateCompat.ACTION_PLAY_PAUSE or
    PlaybackStateCompat.ACTION_SKIP_TO_NEXT or PlaybackStateCompat.ACTION_SKIP_TO_PREVIOUS or
    PlaybackStateCompat.ACTION_SEEK_TO or PlaybackStateCompat.ACTION_STOP or PlaybackStateCompat.ACTION_PLAY_FROM_MEDIA_ID or
    PlaybackStateCompat.ACTION_PLAY_FROM_SEARCH or PlaybackStateCompat.ACTION_SKIP_TO_QUEUE_ITEM

  private fun applyState(next: JSONObject) {
    state = next
    val url = state.optString("art").takeIf { it.isNotEmpty() }
    if (url != artUrl) {
      artUrl = url
      art = null
      // La pochette se charge à part, puis la notification est redessinée avec.
      if (url != null) thread {
        val bitmap = runCatching { URL(url).openStream().use(BitmapFactory::decodeStream) }.getOrNull()
        handler.post {
          if (url == artUrl) {
            art = bitmap
            publish()
          }
        }
      }
    }
    publish()
  }

  private fun publish() {
    if (!state.has("title")) return
    val playing = state.optBoolean("playing")
    session.setMetadata(
      MediaMetadataCompat.Builder()
        .putString(MediaMetadataCompat.METADATA_KEY_TITLE, state.optString("title"))
        .putString(MediaMetadataCompat.METADATA_KEY_ARTIST, state.optString("artist"))
        .putString(MediaMetadataCompat.METADATA_KEY_ALBUM, state.optString("album"))
        .putLong(MediaMetadataCompat.METADATA_KEY_DURATION, state.optLong("duration"))
        .putBitmap(MediaMetadataCompat.METADATA_KEY_ALBUM_ART, art)
        .build(),
    )
    session.setPlaybackState(
      PlaybackStateCompat.Builder()
        .setActions(ACTIONS)
        .setActiveQueueItemId(activeItem)
        .setState(
          if (playing) PlaybackStateCompat.STATE_PLAYING else PlaybackStateCompat.STATE_PAUSED,
          state.optLong("position"),
          if (playing) 1f else 0f,
        )
        .build(),
    )

    fun action(icon: Int, title: String, command: String) = NotificationCompat.Action(
      icon, title,
      PendingIntent.getService(
        this, command.hashCode(),
        Intent(this, MediaService::class.java).setAction(command),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
      ),
    )

    val notification: Notification = NotificationCompat.Builder(this, CHANNEL)
      .setSmallIcon(R.drawable.ic_stat_plexr)
      .setContentTitle(state.optString("title"))
      .setContentText(state.optString("artist"))
      .setSubText(state.optString("album"))
      .setLargeIcon(art)
      .setContentIntent(session.controller.sessionActivity)
      .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
      .setOnlyAlertOnce(true)
      .setOngoing(playing)
      .addAction(action(android.R.drawable.ic_media_previous, "Previous", "previous"))
      .addAction(
        if (playing) action(android.R.drawable.ic_media_pause, "Pause", "pause")
        else action(android.R.drawable.ic_media_play, "Play", "play"),
      )
      .addAction(action(android.R.drawable.ic_media_next, "Next", "next"))
      .setStyle(MediaStyle().setMediaSession(session.sessionToken).setShowActionsInCompactView(0, 1, 2))
      .build()

    runCatching {
      ServiceCompat.startForeground(
        this, NOTIFICATION_ID, notification,
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK else 0,
      )
    }
  }

  /** Maku fermé depuis les applications récentes : la musique de la page s'arrête avec lui (pas celle de la voiture). */
  override fun onTaskRemoved(rootIntent: Intent?) {
    if (player != null) return
    dispatch("stop")
    stopPlayback()
  }

  override fun onDestroy() {
    stopNative()
    instance = null
    session.isActive = false
    session.release()
    ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
    super.onDestroy()
  }
}
