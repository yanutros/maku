package io.github.yanutros.plexr

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.support.v4.media.MediaBrowserCompat.MediaItem
import android.support.v4.media.MediaDescriptionCompat
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder

/**
 * Bibliothèque musicale vue depuis Android Auto, sans la page : Maku peut être fermé quand la
 * voiture s'y connecte. Le compte et le serveur viennent du `config.json` écrit par la partie Rust
 * (même dossier que `app_config_dir`) ; le reste est lu directement auprès du serveur Plex.
 *
 * Identifiants de l'arborescence : `root`, `section:<clé>` (plusieurs bibliothèques musicales),
 * `recent:`, `artists:`, `albums:`, `shuffle:` (suivis de la clé de bibliothèque), `playlists`,
 * `artist:`, `album:`, `playlist:`, `artistshuffle:` (suivis d'un ratingKey), et pour un morceau
 * `track:<conteneur>|<ratingKey>` (le conteneur donne la file d'attente : album, liste…).
 */
class PlexLibrary(private val context: Context) {
  /** `reason` : `config` (réglages illisibles, téléphone jamais déverrouillé depuis l'allumage), `server`, `token`. */
  class NotSignedIn(val reason: String) : Exception(reason)

  /** Morceau prêt à lire. */
  data class Track(
    val ratingKey: String,
    val title: String,
    val artist: String,
    val album: String,
    val durationMs: Long,
    val thumb: String?,
    val partKey: String,
  )

  private data class Server(val base: String, val token: String, val clientId: String)

  private var server: Server? = null

  // ---------- Connexion ----------

  private fun config(): JSONObject {
    val file = File(context.applicationInfo.dataDir, "config.json")
    return runCatching { JSONObject(file.readText()) }.getOrNull() ?: throw NotSignedIn("config")
  }

  /** Adresse du serveur : celle enregistrée par Maku, sinon une autre annoncée par plex.tv (en voiture, hors du Wi-Fi). */
  @Synchronized
  private fun server(): Server {
    server?.let { return it }
    val cfg = config()
    val saved = cfg.optJSONObject("server") ?: throw NotSignedIn("server")
    val clientId = cfg.optString("client_id")
    val token = saved.getString("token")
    val candidates = mutableListOf(saved.getString("uri"))
    if (!reachable(candidates[0], token, clientId)) {
      candidates.clear()
      val account = cfg.optString("plex_token").takeIf { it.isNotEmpty() } ?: throw NotSignedIn("token")
      val resources = JSONArray(get("https://clients.plex.tv/api/v2/resources?includeHttps=1&includeRelay=1", account, clientId))
      for (i in 0 until resources.length()) {
        val res = resources.getJSONObject(i)
        if (res.optString("clientIdentifier") != saved.optString("machine_id")) continue
        val connections = res.optJSONArray("connections") ?: JSONArray()
        val list = (0 until connections.length()).map { connections.getJSONObject(it) }
        // Local d'abord, puis distant, le relais Plex en dernier.
        candidates += list.sortedBy { if (it.optBoolean("relay")) 2 else if (it.optBoolean("local")) 0 else 1 }.map { it.getString("uri") }
      }
    }
    val base = candidates.firstOrNull { reachable(it, token, clientId) } ?: throw java.io.IOException("Plex unreachable")
    return Server(base, token, clientId).also { server = it }
  }

  /** Après une erreur réseau, la prochaine requête cherche à nouveau la bonne adresse. */
  fun forgetServer() {
    server = null
  }

  private fun reachable(base: String, token: String, clientId: String) =
    runCatching { get("$base/identity", token, clientId, timeout = 3000); true }.getOrDefault(false)

  private fun get(url: String, token: String, clientId: String, timeout: Int = 10000): String {
    val conn = URL(url).openConnection() as HttpURLConnection
    conn.connectTimeout = timeout
    conn.readTimeout = timeout
    conn.setRequestProperty("Accept", "application/json")
    conn.setRequestProperty("X-Plex-Token", token)
    conn.setRequestProperty("X-Plex-Client-Identifier", clientId)
    conn.setRequestProperty("X-Plex-Product", "Maku")
    conn.setRequestProperty("X-Plex-Platform", "Android")
    conn.setRequestProperty("X-Plex-Device-Name", "Maku (Android Auto)")
    try {
      if (conn.responseCode !in 200..299) throw java.io.IOException("HTTP ${conn.responseCode}")
      return conn.inputStream.bufferedReader().use { it.readText() }
    } finally {
      conn.disconnect()
    }
  }

  private fun container(path: String): JSONObject {
    val s = server()
    val sep = if ('?' in path) '&' else '?'
    return JSONObject(get("${s.base}$path${sep}X-Plex-Container-Start=0&X-Plex-Container-Size=500", s.token, s.clientId))
      .getJSONObject("MediaContainer")
  }

  private fun metadata(path: String): List<JSONObject> {
    val list = container(path).optJSONArray("Metadata") ?: return emptyList()
    return (0 until list.length()).map { list.getJSONObject(it) }
  }

  // ---------- Images ----------

  /** Image transcodée par le serveur (pour la notification). */
  fun artUrl(thumb: String, size: Int = 400): String {
    val s = server()
    return "${s.base}/photo/:/transcode?width=$size&height=$size&minSize=1&upscale=1&url=${enc(thumb)}&X-Plex-Token=${s.token}"
  }

  /** Android Auto n'affiche que des images locales : elles passent par ArtProvider, réservé à la voiture. */
  private fun artUri(thumb: String?): Uri? {
    if (thumb.isNullOrEmpty()) return null
    val uri = Uri.parse("content://${context.packageName}.art/img").buildUpon().appendQueryParameter("t", thumb).build()
    for (pkg in ART_READERS) runCatching { context.grantUriPermission(pkg, uri, Intent.FLAG_GRANT_READ_URI_PERMISSION) }
    return uri
  }

  // ---------- Arborescence ----------

  fun children(id: String): List<MediaItem> {
    val (kind, arg) = id.split(":", limit = 2).let { it[0] to it.getOrElse(1) { "" } }
    return when (kind) {
      "root" -> {
        val sections = musicSections()
        if (sections.size == 1) categories(sections[0].first, withPlaylists = true)
        else sections.map { (key, title) -> folder("section:$key", title, null) } + playlistsFolder()
      }
      "section" -> categories(arg, withPlaylists = false)
      "recent" -> metadata("/library/sections/$arg/all?type=9&sort=addedAt:desc").take(60).map(::album)
      "albums" -> metadata("/library/sections/$arg/all?type=9&sort=titleSort").map(::album)
      "artists" -> listOf(playable("shuffle:$arg", context.getString(R.string.auto_shuffle), null, null)) +
        metadata("/library/sections/$arg/all?type=8&sort=titleSort").map {
        folder("artist:${it.getString("ratingKey")}", it.optString("title"), it.thumb(), grid = true)
      }
      "artist" -> listOf(playable("artistshuffle:$arg", context.getString(R.string.auto_shuffle), null, null)) +
        metadata("/library/metadata/$arg/children").map(::album)
      "playlists" -> metadata("/playlists?playlistType=audio").map {
        MediaItem(
          describe("playlist:${it.getString("ratingKey")}", it.optString("title"), null, it.thumb()),
          MediaItem.FLAG_BROWSABLE or MediaItem.FLAG_PLAYABLE,
        )
      }
      "album", "playlist" -> tracks(id).map { t ->
        playable("track:$id|${t.ratingKey}", t.title, t.artist, t.thumb)
      }
      else -> emptyList()
    }
  }

  private fun musicSections(): List<Pair<String, String>> {
    val dirs = container("/library/sections").optJSONArray("Directory") ?: JSONArray()
    return (0 until dirs.length()).map { dirs.getJSONObject(it) }
      .filter { it.optString("type") == "artist" }
      .map { it.getString("key") to it.optString("title") }
  }

  // Android Auto montre au plus quatre onglets : la lecture aléatoire ouvre la liste des artistes.
  private fun categories(section: String, withPlaylists: Boolean): List<MediaItem> = listOf(
    folder("recent:$section", context.getString(R.string.auto_recent), null, grid = true, icon = R.drawable.ic_auto_home),
    folder("artists:$section", context.getString(R.string.auto_artists), null, icon = R.drawable.ic_auto_artists),
    folder("albums:$section", context.getString(R.string.auto_albums), null, grid = true, icon = R.drawable.ic_auto_albums),
  ) + (if (withPlaylists) playlistsFolder() else emptyList())

  private fun playlistsFolder() = listOf(folder("playlists", context.getString(R.string.auto_playlists), null, icon = R.drawable.ic_auto_playlists))

  private fun album(it: JSONObject) = MediaItem(
    describe("album:${it.getString("ratingKey")}", it.optString("title"), it.optString("parentTitle"), it.thumb()),
    MediaItem.FLAG_BROWSABLE or MediaItem.FLAG_PLAYABLE,
  )

  /** `icon` : pictogramme d'un onglet (Android Auto l'affiche à la place du titre). */
  private fun folder(id: String, title: String, thumb: String?, grid: Boolean = false, icon: Int = 0): MediaItem {
    val description = describe(id, title, null, thumb, if (grid) 2 else 1)
    if (icon == 0) return MediaItem(description, MediaItem.FLAG_BROWSABLE)
    val uri = Uri.parse("android.resource://${context.packageName}/$icon")
    return MediaItem(
      MediaDescriptionCompat.Builder().setMediaId(id).setTitle(title).setIconUri(uri).setExtras(description.extras).build(),
      MediaItem.FLAG_BROWSABLE,
    )
  }

  private fun playable(id: String, title: String, subtitle: String?, thumb: String?) =
    MediaItem(describe(id, title, subtitle, thumb), MediaItem.FLAG_PLAYABLE)

  /** `childStyle` : 1 = liste, 2 = grille, pour les éléments contenus dans ce dossier. */
  private fun describe(id: String, title: String, subtitle: String?, thumb: String?, childStyle: Int = 0): MediaDescriptionCompat {
    val builder = MediaDescriptionCompat.Builder().setMediaId(id).setTitle(title).setSubtitle(subtitle).setIconUri(artUri(thumb))
    if (childStyle != 0) {
      builder.setExtras(
        Bundle().apply {
          putInt("android.media.browse.CONTENT_STYLE_BROWSABLE_HINT", childStyle)
          putInt("android.media.browse.CONTENT_STYLE_PLAYABLE_HINT", childStyle)
        },
      )
    }
    return builder.build()
  }

  // ---------- Lecture ----------

  /** File d'attente pour un élément choisi dans la voiture, et le morceau par lequel commencer. */
  fun queue(id: String): Pair<List<Track>, Int> {
    val (kind, arg) = id.split(":", limit = 2).let { it[0] to it.getOrElse(1) { "" } }
    return when (kind) {
      "track" -> {
        val (parent, key) = arg.split("|", limit = 2).let { it[0] to it.getOrElse(1) { "" } }
        val list = tracks(parent)
        list to list.indexOfFirst { it.ratingKey == key }.coerceAtLeast(0)
      }
      "shuffle" -> metadata("/library/sections/$arg/all?type=10&sort=random").take(200).mapNotNull(::track) to 0
      "artistshuffle" -> metadata("/library/metadata/$arg/allLeaves").mapNotNull(::track).shuffled() to 0
      else -> tracks(id) to 0
    }
  }

  private fun tracks(id: String): List<Track> {
    val (kind, key) = id.split(":", limit = 2).let { it[0] to it.getOrElse(1) { "" } }
    val path = when (kind) {
      "album" -> "/library/metadata/$key/children"
      "playlist" -> "/playlists/$key/items"
      "artist" -> "/library/metadata/$key/allLeaves"
      else -> return emptyList()
    }
    return metadata(path).mapNotNull(::track)
  }

  /** Recherche vocale (« Joue … sur Maku ») : le premier artiste, album ou morceau trouvé. */
  fun search(query: String): Pair<List<Track>, Int> {
    if (query.isBlank()) return musicSections().firstOrNull()?.let { queue("shuffle:${it.first}") } ?: (emptyList<Track>() to 0)
    val hubs = container("/hubs/search?query=${enc(query)}&limit=5").optJSONArray("Hub") ?: JSONArray()
    for (i in 0 until hubs.length()) {
      val hub = hubs.getJSONObject(i)
      val first = hub.optJSONArray("Metadata")?.optJSONObject(0) ?: continue
      val key = first.getString("ratingKey")
      when (hub.optString("type")) {
        "artist" -> return tracks("artist:$key").shuffled() to 0
        "album" -> return tracks("album:$key") to 0
        "track" -> track(first)?.let { return listOf(it) to 0 }
      }
    }
    return emptyList<Track>() to 0
  }

  private fun track(it: JSONObject): Track? {
    val part = it.optJSONArray("Media")?.optJSONObject(0)?.optJSONArray("Part")?.optJSONObject(0)?.optString("key")
    if (part.isNullOrEmpty()) return null
    return Track(
      ratingKey = it.getString("ratingKey"),
      title = it.optString("title"),
      artist = it.optString("originalTitle").ifEmpty { it.optString("grandparentTitle") },
      album = it.optString("parentTitle"),
      durationMs = it.optLong("duration"),
      thumb = it.optString("parentThumb").ifEmpty { it.optString("thumb") }.ifEmpty { null },
      partKey = part,
    )
  }

  fun streamUrl(t: Track): String {
    val s = server()
    return "${s.base}${t.partKey}?X-Plex-Token=${s.token}&X-Plex-Client-Identifier=${s.clientId}"
  }

  /** Progression envoyée à Plex (historique, « vu », statistiques), comme le fait la page. */
  fun timeline(t: Track, state: String, positionMs: Long) {
    runCatching {
      val s = server()
      get(
        "${s.base}/:/timeline?ratingKey=${t.ratingKey}&key=${enc("/library/metadata/${t.ratingKey}")}" +
          "&state=$state&time=$positionMs&duration=${t.durationMs}",
        s.token, s.clientId,
      )
    }
  }

  private fun JSONObject.thumb(): String? = optString("thumb").ifEmpty { optString("composite") }.ifEmpty { null }

  companion object {
    /** Android Auto (et son simulateur de bureau) lit les pochettes. */
    val ART_READERS = listOf("com.google.android.projection.gearhead", "com.google.android.autosimulator")

    private fun enc(s: String) = URLEncoder.encode(s, "UTF-8")
  }
}
