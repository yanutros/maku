package io.github.yanutros.plexr

import android.content.Context
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Journal d'Android Auto (cache de l'application, 60 lignes au plus) : ce que la voiture a demandé
 * et les erreurs rencontrées, lisible dans Réglages → Compte sur le téléphone, pour comprendre
 * après coup un échec en voiture.
 */
object AutoLog {
  private const val MAX_LINES = 60

  private fun file(context: Context) = File(context.cacheDir, "android-auto.log")

  @Synchronized
  fun append(context: Context, line: String) {
    runCatching {
      val f = file(context)
      val stamp = SimpleDateFormat("dd/MM HH:mm:ss", Locale.ROOT).format(Date())
      val lines = (if (f.exists()) f.readLines() else emptyList()) + "$stamp  $line"
      f.writeText(lines.takeLast(MAX_LINES).joinToString("\n") + "\n")
    }
  }

  fun read(context: Context): String = runCatching { file(context).readText() }.getOrDefault("")
}
