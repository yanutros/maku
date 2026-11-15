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

/**
 * L'écran reste-t-il allumé ? Tant que la lecture est demandée (en cours, ou en chargement), pas en pause ni une
 * fois le fichier fini : sur un téléviseur OLED, une image fixe ne doit pas empêcher l'économiseur d'écran, et un
 * téléphone en pause doit pouvoir s'éteindre.
 */
fun keepsScreenOn(playWhenReady: Boolean, state: Int): Boolean =
  playWhenReady && (state == androidx.media3.common.Player.STATE_READY || state == androidx.media3.common.Player.STATE_BUFFERING)

/**
 * Mémoire tampon du lecteur vidéo, en octets, d'après la mémoire que l'appareil accorde à Maku (`memoryClassMb`).
 * Le réglage par défaut du lecteur (jusqu'à 144 Mo) est pris sur cette mémoire-là : un téléviseur qui en accorde
 * 192 Mo y laissait les trois quarts avec un film à très haut débit, au risque de s'arrêter faute de mémoire.
 * Jamais plus de 45 % de la mémoire accordée, jamais moins de 32 Mo, et jamais plus que le réglage par défaut.
 */
fun videoBufferBytes(memoryClassMb: Int): Int {
  val mb = (memoryClassMb * 45 / 100).coerceIn(MIN_VIDEO_BUFFER_MB, DEFAULT_VIDEO_BUFFER_MB)
  return mb * 1024 * 1024
}

const val MIN_VIDEO_BUFFER_MB = 32
/** Réglage par défaut du lecteur : 131 Mo pour l'image et 13 Mo pour le son. */
const val DEFAULT_VIDEO_BUFFER_MB = 144

/**
 * Commandes du service de musique (MediaService.kt) : celles de la notification, de la voiture ou d'un casque.
 * Le service est visible des autres applications (Android Auto doit pouvoir s'y relier) : une action qui n'est
 * pas l'une de celles-ci est ignorée, et rien d'autre n'est jamais écrit dans le script envoyé à la page.
 */
object MediaActions {
  private val SIMPLE = setOf("play", "pause", "next", "previous", "stop")
  /** Position en millisecondes : des chiffres seulement, et pas plus qu'un `Long` n'en contient. */
  private val SEEK = Regex("""seek:\d{1,15}""")

  /** L'action, si c'en est une ; sinon rien. */
  fun valid(action: String?): String? = action?.takeIf { it in SIMPLE || SEEK.matches(it) }

  /** Script qui transmet l'action à la page, ou rien si elle n'est pas reconnue. */
  fun script(action: String?): String? = valid(action)?.let { "window.__plexrMedia && window.__plexrMedia('$it')" }

  /**
   * L'application `client` peut-elle parcourir la bibliothèque et y lancer une lecture ? Maku lui-même, celles de la
   * liste (Android Auto, l'Assistant) et le système. Les autres gardent les commandes de la lecture en cours
   * (pause, suivant…), comme un casque, mais ne font rien jouer.
   */
  fun trusted(client: String?, own: String, allowed: Collection<String>, uid: Int, systemUid: Int): Boolean =
    uid == systemUid || (client != null && (client == own || client in allowed))
}

/**
 * Porte du pont entre la page et Android (`MakuBridge`).
 *
 * Android montre l'objet du pont à toute page affichée dans le WebView, y compris dans un cadre : la bande-annonce
 * d'un autre site pouvait donc lire le journal, piloter le lecteur ou ouvrir le partage. La page de Maku tire une
 * clé au hasard et la présente en tout premier, avant d'afficher quoi que ce soit ; cette clé est retenue, aucune
 * autre ne le sera, et chaque méthode du pont la demande. Un cadre ne peut ni la lire (elle vit dans la page de
 * Maku, d'une autre origine) ni en présenter une autre.
 */
class BridgeGate {
  private var key: String? = null

  /** Première clé présentée : retenue. Ensuite : vrai pour elle seule (la page rechargée la présente de nouveau). */
  @Synchronized
  fun claim(candidate: String?): Boolean {
    if (candidate == null || !KEY.matches(candidate)) return false
    val current = key ?: return true.also { key = candidate }
    return same(current, candidate)
  }

  @Synchronized
  fun allows(candidate: String?): Boolean {
    val current = key ?: return false
    return candidate != null && same(current, candidate)
  }

  private fun same(a: String, b: String) = java.security.MessageDigest.isEqual(a.toByteArray(), b.toByteArray())

  private companion object {
    /** 128 bits au moins, en hexadécimal. */
    val KEY = Regex("[0-9a-f]{32,64}")
  }
}

/**
 * Ce que la page peut demander à l'appareil (MainActivity.kt) : un nom d'image à partager, un APK à installer.
 * Le pont est visible de toute page affichée dans Maku (voir BridgeGate, qui le ferme aux autres) : en plus, rien
 * de ce qu'il reçoit ne doit pouvoir désigner un autre fichier que ceux prévus.
 */
object BridgeRules {
  private val NAME = Regex("[A-Za-z0-9_-]{1,40}")

  /** Nom de l'image partagée : lettres, chiffres et tirets ; sinon un nom fixe (jamais un chemin). */
  fun shareName(name: String?): String = name?.takeIf { NAME.matches(it) } ?: "maku"

  /** L'APK à installer est-il bien une mise à jour téléchargée par Maku (dossier `updates` de son cache) ? */
  fun isUpdateApk(path: String?, cacheDir: java.io.File): Boolean {
    if (path.isNullOrEmpty() || !path.endsWith(".apk", ignoreCase = true)) return false
    val updates = runCatching { java.io.File(cacheDir, "updates").canonicalFile }.getOrNull() ?: return false
    val file = runCatching { java.io.File(path).canonicalFile }.getOrNull() ?: return false
    return file.parentFile == updates
  }
}

/**
 * Ce que le journal de diagnostic (DiagLog.kt) retire d'une ligne avant de l'écrire : adresses de serveur (le chemin
 * est gardé, il dit si le serveur convertit), jetons, adresses IP, identifiants et chemins de fichiers.
 */
object DiagClean {
  private val URL = Regex("""https?://[^/\s"'<>]+(/[^\s"'<>?]*)?(\?[^\s"'<>]*)?""")
  private val FILE = Regex("""file://[^\s"'<>]+""")
  private val SECRET = Regex("""(?i)(api_key|x-plex-token|token|accesstoken|deviceid|userid|playsessionid|mediasourceid)=[^&\s"']+""")
  /** Un jeton écrit autrement qu'en « clé=valeur » : en-tête (« X-Plex-Token: … ») ou JSON (« "AccessToken":"…" »). */
  private val HEADER = Regex("""(?i)(x-plex-token|x-emby-token|x-mediabrowser-token|x-maku-token|access_?token|api_?key|authorization)["']?\s*:\s*["']?[^\s"',;}]+""")
  private val IP = Regex("""\b\d{1,3}(\.\d{1,3}){3}\b""")
  /** IPv6 : au moins quatre groupes (une heure « 20:12:34 » n'en a que trois), ou la forme abrégée « :: ». */
  private val IP6 = Regex("""(?i)(?<![0-9a-f:.])(?:(?:[0-9a-f]{1,4}:){3,7}[0-9a-f]{1,4}|(?:[0-9a-f]{1,4}:){1,6}:(?:[0-9a-f]{1,4}(?::[0-9a-f]{1,4}){0,5})?)(?![0-9a-f:])""")
  /** Identifiants : de 32 à 64 chiffres hexadécimaux (Jellyfin, Plex), ou un UUID avec ses tirets. */
  private val ID = Regex("""(?i)\b[0-9a-f]{32,64}\b|\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b""")

  /** `hosts` : noms de serveur déjà vus, masqués aussi hors d'une adresse (« Unable to resolve host … »). */
  fun clean(text: String, hosts: Collection<String> = emptyList()): String = text
    .replace(URL) { "<serveur>" + (it.groupValues[1].ifEmpty { "/" }) }
    .replace(FILE, "<fichier>")
    .let { t -> hosts.fold(t) { acc, h -> acc.replace(h, "<serveur>", ignoreCase = true) } }
    .replace(SECRET) { it.value.substringBefore('=') + "=…" }
    .replace(HEADER) { it.groupValues[1] + ": …" }
    .replace(ID, "<id>")
    .replace(IP, "<ip>")
    .replace(IP6, "<ip>")
}
