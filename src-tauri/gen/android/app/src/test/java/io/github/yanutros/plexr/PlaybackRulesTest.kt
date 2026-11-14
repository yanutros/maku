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
