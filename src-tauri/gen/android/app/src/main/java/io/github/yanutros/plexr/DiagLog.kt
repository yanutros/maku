package io.github.yanutros.plexr

import android.app.ActivityManager
import android.content.Context
import android.os.Build
import android.os.Debug
import android.os.Handler
import android.os.HandlerThread
import android.os.SystemClock
import android.view.WindowManager
import android.webkit.WebView
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Journal de diagnostic, à transmettre quand l'appareil se bloque ou qu'une lecture échoue
 * (Réglages → Lecteur → Journal de diagnostic).
 *
 * Il note l'appareil, chaque lecture (fichier d'origine ou conversion, décodeurs choisis, format de
 * l'image), les erreurs, et la mémoire libre toutes les 30 secondes pendant une vidéo. Chaque ligne
 * est écrite sur le disque aussitôt : celles qui précèdent un blocage survivent au redémarrage de
 * l'appareil. Rien de personnel : adresses de serveur, jetons et identifiants sont masqués avant
 * l'écriture. Deux fichiers de 300 Ko au plus (le courant et le précédent).
 */
object DiagLog {
  private const val MAX_BYTES = 300 * 1024L
  private const val PREFS = "maku.diag"
  private const val SAMPLE_MS = 30_000L

  private val thread = HandlerThread("maku-diag").apply { start() }
  private val handler = Handler(thread.looper)
  private val time = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.ROOT)
  private var dir: File? = null
  private var app: Context? = null
  @Volatile private var sampling = false
  private val sampler = Runnable { sample() }

  /** Au lancement : décrit l'appareil, et signale une lecture précédente interrompue (blocage, plantage). */
  fun init(context: Context) {
    if (app != null) return
    val ctx = context.applicationContext
    app = ctx
    dir = File(ctx.filesDir, "diag").apply { mkdirs() }
    val prefs = ctx.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    val interrupted = prefs.getString("playing", null)
    prefs.edit().remove("playing").apply()
    log("──────── Lancement de Maku ────────")
    log(deviceInfo(ctx))
    log("Appareil allumé depuis ${SystemClock.elapsedRealtime() / 60_000} min")
    if (interrupted != null) {
      log("⚠ La lecture précédente ne s'est pas terminée normalement (commencée le $interrupted) : Maku ou l'appareil s'est arrêté pendant la vidéo")
    }
    memory("au lancement")
  }

  /** Heure prise à l'appel, pour garder les lignes dans l'ordre ; SimpleDateFormat ne se partage pas entre fils. */
  private fun stamp(): String = synchronized(time) { time.format(Date()) }

  fun log(message: String) {
    val line = "${stamp()}  ${clean(message)}\n"
    handler.post { write(line) }
  }

  /** Une vidéo commence : marque posée sur le disque (retirée à la fin) et mémoire suivie. */
  fun playbackStarted() {
    app?.getSharedPreferences(PREFS, Context.MODE_PRIVATE)?.edit()?.putString("playing", stamp())?.commit()
    if (sampling) return
    sampling = true
    handler.postDelayed(sampler, SAMPLE_MS)
  }

  fun playbackStopped() {
    app?.getSharedPreferences(PREFS, Context.MODE_PRIVATE)?.edit()?.remove("playing")?.apply()
    sampling = false
    handler.removeCallbacks(sampler)
    memory("fin de la vidéo")
  }

  private fun sample() {
    if (!sampling) return
    memory("pendant la vidéo")
    handler.postDelayed(sampler, SAMPLE_MS)
  }

  /** Mémoire de l'appareil et de Maku (dont la mémoire graphique, celle des images décodées). */
  fun memory(context: String) {
    val at = stamp()
    handler.post { memoryAt(at, context) }
  }

  private fun memoryAt(at: String, context: String) {
    val ctx = app ?: return
    val am = ctx.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
    val mi = ActivityManager.MemoryInfo().also { am.getMemoryInfo(it) }
    val dm = Debug.MemoryInfo().also { Debug.getMemoryInfo(it) }
    val mb = { kb: Int -> kb / 1024 }
    val graphics = dm.getMemoryStat("summary.graphics")?.toIntOrNull()?.takeIf { it > 0 }?.let { " · graphique ${mb(it)} Mo" } ?: ""
    val java = (Runtime.getRuntime().totalMemory() - Runtime.getRuntime().freeMemory()) / 1_048_576
    write(
      "$at  Mémoire ($context) : appareil ${mi.availMem / 1_048_576} Mo libres sur ${mi.totalMem / 1_048_576} Mo" +
        (if (mi.lowMemory) " ⚠ MÉMOIRE BASSE" else "") +
        " · Maku ${mb(dm.totalPss)} Mo$graphics · Java $java Mo · natif ${Debug.getNativeHeapAllocatedSize() / 1_048_576} Mo" +
        " · ${Thread.activeCount()} fils\n",
    )
  }

  /** Les deux fichiers, du plus ancien au plus récent. */
  fun text(): String {
    val d = dir ?: return ""
    val parts = listOf(File(d, "maku.1.log"), File(d, "maku.log")).filter { it.exists() }
    return parts.joinToString("") { runCatching { it.readText() }.getOrDefault("") }
  }

  /** Copie du journal dans le cache, à partager (FileProvider). */
  fun exportFile(context: Context): File =
    File(context.cacheDir, "maku-journal.txt").apply { writeText(text()) }

  private fun write(line: String) {
    val d = dir ?: return
    val file = File(d, "maku.log")
    runCatching {
      if (file.length() > MAX_BYTES) file.renameTo(File(d, "maku.1.log").apply { delete() })
      // Ouvert, écrit et refermé à chaque ligne : rien ne reste en attente si l'appareil se bloque.
      file.appendText(line)
    }
  }

  private fun deviceInfo(ctx: Context): String {
    val am = ctx.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
    val mi = ActivityManager.MemoryInfo().also { am.getMemoryInfo(it) }
    val version = runCatching { ctx.packageManager.getPackageInfo(ctx.packageName, 0).versionName }.getOrNull()
    val soc = if (Build.VERSION.SDK_INT >= 31) " · puce ${Build.SOC_MANUFACTURER} ${Build.SOC_MODEL}" else " · puce ${Build.HARDWARE}"
    val webView = if (Build.VERSION.SDK_INT >= 26) WebView.getCurrentWebViewPackage()?.let { " · WebView ${it.versionName}" } ?: "" else ""
    @Suppress("DEPRECATION")
    val display = (ctx.getSystemService(Context.WINDOW_SERVICE) as WindowManager).defaultDisplay
    val mode = if (Build.VERSION.SDK_INT >= 23) display.mode.let { " · écran ${it.physicalWidth}×${it.physicalHeight} à ${"%.0f".format(it.refreshRate)} Hz" } else ""
    val hdr = if (Build.VERSION.SDK_INT >= 24) {
      display.hdrCapabilities?.supportedHdrTypes?.joinToString(",") { t ->
        when (t) { 1 -> "DolbyVision"; 2 -> "HDR10"; 3 -> "HLG"; 4 -> "HDR10+"; else -> "$t" }
      }?.takeIf { it.isNotEmpty() }?.let { " · HDR $it" } ?: ""
    } else ""
    return "Maku $version · ${Build.MANUFACTURER} ${Build.MODEL} (${Build.DEVICE})$soc · Android ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT})" +
      " · mémoire ${mi.totalMem / 1_048_576} Mo" + (if (am.isLowRamDevice) " (appareil à mémoire réduite)" else "") +
      " · Maku peut utiliser ${am.memoryClass}/${am.largeMemoryClass} Mo$webView$mode$hdr"
  }

  private val URL = Regex("""https?://[^/\s"'<>]+(/[^\s"'<>?]*)?(\?[^\s"'<>]*)?""")
  private val SECRET = Regex("""(?i)(api_key|x-plex-token|token|accesstoken|deviceid|userid|playsessionid|mediasourceid)=[^&\s"']+""")
  private val IP = Regex("""\b\d{1,3}(\.\d{1,3}){3}\b""")
  private val ID = Regex("""\b[0-9a-f]{32}\b""", RegexOption.IGNORE_CASE)

  /** Noms de serveur vus dans les lectures, masqués aussi dans les messages d’erreur (« Unable to resolve host … »). */
  private val hosts = java.util.concurrent.CopyOnWriteArraySet<String>()

  fun hide(host: String?) {
    if (!host.isNullOrBlank()) hosts.add(host)
  }

  /** Masque ce qui pourrait identifier l'utilisateur ou son serveur ; garde le chemin (il dit si le serveur convertit). */
  fun clean(text: String): String = text
    .replace(URL) { "<serveur>" + (it.groupValues[1].ifEmpty { "/" }) }
    .let { t -> hosts.fold(t) { acc, h -> acc.replace(h, "<serveur>", ignoreCase = true) } }
    .replace(SECRET) { it.value.substringBefore('=') + "=…" }
    .replace(IP, "<ip>")
    .replace(ID, "<id>")
}
