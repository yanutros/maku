package io.github.yanutros.plexr

import androidx.media3.common.C
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException

/**
 * Règles du lecteur vidéo natif (NativeVideo.kt) qui ne dépendent ni d'Android ni d'ExoPlayer : elles sont
 * vérifiées par des tests ordinaires (app/src/test), sans appareil. Seules des constantes de Media3 y figurent.
 */

/**
 * Vidéo figée : le lecteur se dit prêt, n'est pas en pause, a au moins 2 s de données d'avance, mais la position
 * ne bouge plus depuis 5 s. Une coupure réseau n'en est pas une (le lecteur est alors en chargement, sans avance).
 * Appelée toutes les 500 ms ; chaque appel dit ce qui vient de changer, ou rien.
 */
class StallWatch {
  sealed interface Event
  /** La lecture est figée à `at` (ms) depuis `frozenMs`. */
  data class Stalled(val at: Long, val frozenMs: Long) : Event
  /** Un petit pas en avant après un blocage : la lecture a vraiment repris. */
  data class Resumed(val at: Long, val frozenMs: Long) : Event
  /** Un grand écart pendant un blocage : un saut (Passer 10 s…), qui sera de nouveau surveillé. */
  data class Jumped(val at: Long) : Event

  /** Blocage déjà signalé. */
  var stalled = false
    private set
  /** Position surveillée, et depuis quand elle n'a pas bougé (en lecture). */
  private var pos = -1L
  private var since = 0L

  /** Nouvelle lecture, ou relance au même endroit : la surveillance repart de zéro. */
  fun reset() {
    stalled = false
    pos = -1L
  }

  /** `playing` : prêt et pas en pause ; `buffered` : position jusqu'où les données sont chargées (ms). */
  fun update(now: Long, position: Long, playing: Boolean, buffered: Long): Event? {
    if (pos < 0 || kotlin.math.abs(position - pos) > MOVED_MS) {
      var event: Event? = null
      if (stalled) {
        val step = position - pos
        event = if (step in 1..RESUME_STEP_MS) Resumed(position, now - since) else Jumped(position)
        stalled = false
      }
      pos = position
      since = now
      return event
    }
    // En pause ou en chargement, le temps ne compte pas.
    if (!playing || buffered - position < AHEAD_MS) {
      if (!stalled) since = now
      return null
    }
    if (stalled || now - since < FROZEN_MS) return null
    stalled = true
    return Stalled(position, now - since)
  }

  companion object {
    /** Écart en deçà duquel la position est considérée immobile. */
    const val MOVED_MS = 300L
    /** Données d'avance minimales pour parler de blocage (sinon, c'est un chargement). */
    const val AHEAD_MS = 2000L
    /** Durée d'immobilité avant de signaler le blocage. */
    const val FROZEN_MS = 5000L
    /** Pas en avant maximal pour une reprise (au-delà, un saut). */
    const val RESUME_STEP_MS = 3000L
  }
}

/**
 * Son envoyé tel quel à la sortie audio (Dolby, DTS…) : le réglage automatique le permet, aucun décodeur son ne
 * s'est ouvert ici, et le format du son est connu. Certaines TV calent dessus après un saut ; la page passe alors
 * en « Décodé ici ».
 */
fun soundSentAsIs(passthroughOn: Boolean, audioDecoded: Boolean, audioMime: String?) =
  passthroughOn && !audioDecoded && audioMime != null

/**
 * Garde le format de l'image (bandes noires en haut et en bas pour un film en 2.39:1, comme dans Plex) : la plus
 * grande taille qui tient dans l'écran. Rien tant que l'image ou l'écran n'ont pas de taille.
 */
fun fitVideo(videoWidth: Int, videoHeight: Int, pixelRatio: Float, screenWidth: Int, screenHeight: Int): Pair<Int, Int>? {
  if (videoWidth == 0 || videoHeight == 0 || screenWidth == 0 || screenHeight == 0) return null
  val ratio = videoWidth * pixelRatio / videoHeight
  return if (screenWidth.toFloat() / screenHeight > ratio) {
    (screenHeight * ratio).toInt() to screenHeight
  } else {
    screenWidth to (screenWidth / ratio).toInt()
  }
}

/** Type d'un fichier de sous-titres externe, d'après le codec annoncé par le serveur (SRT sinon). */
fun subtitleMime(codec: String) = when (codec.lowercase()) {
  "ass", "ssa" -> MimeTypes.TEXT_SSA
  "vtt", "webvtt" -> MimeTypes.TEXT_VTT
  "ttml", "dfxp" -> MimeTypes.APPLICATION_TTML
  else -> MimeTypes.APPLICATION_SUBRIP
}

/** Erreur de décodage (format ou codec que l'appareil ne sait pas lire) : la page demande alors la conversion. */
fun isDecodingError(errorCode: Int) =
  errorCode in 4000..4999 || errorCode == PlaybackException.ERROR_CODE_PARSING_CONTAINER_UNSUPPORTED

/** Piste demandée par la page, numérotée comme dans mpv : « no » (aucune), ou un numéro à partir de 1. */
sealed interface TrackChoice {
  data object Off : TrackChoice
  /** Position dans les pistes de ce type (à partir de 0). */
  data class At(val index: Int) : TrackChoice

  companion object {
    /** Rien pour une valeur illisible (ou 0) : la sélection reste telle quelle. */
    fun parse(value: String): TrackChoice? {
      if (value == "no") return Off
      val n = value.toIntOrNull() ?: return null
      return if (n >= 1) At(n - 1) else null
    }
  }
}

/**
 * Comme Moonfin : au démarrage à froid, la sortie audio peut répondre qu'elle n'accepte rien tel quel, le temps que
 * la liaison HDMI avec l'ampli s'établisse — le son serait alors décodé ici, sans Dolby ni DTS. La dernière réponse
 * valable est gardée (pour ce système : une mise à jour l'efface) et reprise dans les minutes qui suivent le
 * démarrage, si la réponse du moment est vide. Une vraie réponse, même différente (barre de son débranchée), la
 * remplace : elle n'est jamais écrasée par une réponse vide.
 */
object AudioMemory {
  /** Après le démarrage de l'appareil, la liaison HDMI peut ne pas être encore établie. */
  const val BOOT_WINDOW_MS = 10 * 60 * 1000L

  sealed interface Choice
  /** La sortie accepte des formats tels quels : on les garde en mémoire, et on la laisse répondre elle-même. */
  data class Remember(val encodings: List<Int>, val channels: Int) : Choice
  /** Rien à reprendre : la réponse du moment fait foi. */
  data object Live : Choice
  /** Réponse vide juste après le démarrage : on reprend la dernière valable (le son décodé ici reste possible). */
  data class Saved(val encodings: IntArray, val channels: Int) : Choice {
    override fun equals(other: Any?) = other is Saved && encodings.contentEquals(other.encodings) && channels == other.channels
    override fun hashCode() = 31 * encodings.contentHashCode() + channels
  }

  /**
   * `liveEncodings` : formats que la sortie accepte tels quels en ce moment ; `savedEncodings` : ceux gardés, séparés
   * par des virgules, pour le système `savedBuild`.
   */
  fun choose(
    liveEncodings: List<Int>,
    liveChannels: Int,
    sinceBootMs: Long,
    build: String,
    savedBuild: String?,
    savedEncodings: String?,
    savedChannels: Int,
  ): Choice {
    if (liveEncodings.isNotEmpty()) return Remember(liveEncodings, liveChannels)
    if (sinceBootMs > BOOT_WINDOW_MS || savedBuild != build) return Live
    val saved = savedEncodings?.split(",")?.mapNotNull { it.toIntOrNull() } ?: return Live
    if (saved.isEmpty()) return Live
    return Saved((listOf(C.ENCODING_PCM_16BIT) + saved).toIntArray(), savedChannels)
  }
}
