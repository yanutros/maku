package io.github.yanutros.plexr

import android.app.UiModeManager
import android.content.ComponentName
import android.content.Intent
import android.content.pm.ActivityInfo
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.content.FileProvider
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.core.view.WindowInsetsCompat
import java.io.File

class MainActivity : TauriActivity() {
  private var video: NativeVideo? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // Android 17 : sans cette autorisation, les serveurs du réseau local (Jellyfin, Plex en local)
    // sont injoignables.
    if (Build.VERSION.SDK_INT >= 37 && checkSelfPermission(LOCAL_NETWORK) != PackageManager.PERMISSION_GRANTED) {
      requestPermissions(arrayOf(LOCAL_NETWORK), 0)
    }
  }

  private companion object {
    const val LOCAL_NETWORK = "android.permission.ACCESS_LOCAL_NETWORK"
  }

  /**
   * Tauri met la page en pause quand Maku passe en arrière-plan, ce qui coupe la musique : tant
   * qu'un morceau joue, la page est aussitôt relancée (la notification garde Maku en vie).
   */
  override fun onPause() {
    super.onPause()
    if (MediaService.playing) MediaService.webView?.onResume()
  }

  /** Maku quitté (bouton Accueil) : la vidéo se met en pause, comme dans Plex. */
  override fun onStop() {
    super.onStop()
    video?.command("pause", "")
  }

  // Le WebView annonce un écran tactile même sur téléviseur : l'interface demande donc à
  // Android (`window.PlexrDevice.isTv()`) si l'appareil est en mode télévision.
  override fun onWebViewCreate(webView: WebView) {
    val tv = (getSystemService(UI_MODE_SERVICE) as UiModeManager).currentModeType == Configuration.UI_MODE_TYPE_TELEVISION ||
      packageManager.hasSystemFeature("android.software.leanback_only")
    // Sur téléviseur, la page fixe sa propre largeur (balise viewport) et le WebView la réduit à l'écran.
    if (tv) {
      webView.settings.useWideViewPort = true
      webView.settings.loadWithOverviewMode = true
    }
    // Sur téléviseur, la page reçoit les touches dès le premier appui sur la télécommande.
    if (tv) webView.post { webView.requestFocus() }
    MediaService.webView = webView
    val video = NativeVideo(this, webView).also { this.video = it }
    webView.addJavascriptInterface(object {
      /** Vidéo : lecteur natif ExoPlayer sous la page (voir NativeVideo.kt). */
      @JavascriptInterface
      fun videoOpen(json: String) {
        video.open(json)
      }

      @JavascriptInterface
      fun videoCommand(name: String, value: String) {
        video.command(name, value)
      }

      @JavascriptInterface
      fun videoClose() {
        video.closeAsync()
      }

      @JavascriptInterface
      fun isTv(): Boolean = tv


      /** Téléviseur : OK sur un champ de saisie ouvre le clavier (il ne s'ouvre pas au simple passage). */
      @JavascriptInterface
      fun showKeyboard() {
        runOnUiThread {
          val imm = getSystemService(INPUT_METHOD_SERVICE) as android.view.inputmethod.InputMethodManager
          // Le champ vient de changer de mode de saisie : Android le relit avant d'ouvrir le clavier.
          imm.restartInput(webView)
          webView.postDelayed({ imm.showSoftInput(webView, android.view.inputmethod.InputMethodManager.SHOW_FORCED) }, 100)
        }
      }

      /** Musique : état de la lecture pour la notification (JSON, voir MediaService). */
      @JavascriptInterface
      fun mediaUpdate(json: String) = MediaService.update(this@MainActivity, json)

      @JavascriptInterface
      fun mediaStop() = MediaService.stop()

      /** Téléphone : vidéo en paysage et plein écran (barres système masquées), ou retour à la normale. */
      @JavascriptInterface
      fun setVideoMode(on: Boolean) {
        runOnUiThread {
          requestedOrientation = if (on) ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE else ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
          val controller = WindowCompat.getInsetsController(window, window.decorView)
          if (on) {
            controller.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
            controller.hide(WindowInsetsCompat.Type.systemBars())
          } else {
            controller.show(WindowInsetsCompat.Type.systemBars())
          }
        }
      }

      /** Hauteur des barres d'état et de navigation (« haut,bas », en px CSS), que la page évite. */
      @JavascriptInterface
      fun insets(): String {
        val insets = ViewCompat.getRootWindowInsets(webView)
          ?.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
          ?: return "0,0"
        val density = resources.displayMetrics.density
        return "${(insets.top / density).toInt()},${(insets.bottom / density).toInt()}"
      }

      /** Icône du lanceur aux couleurs du thème : active l'alias correspondant, désactive les autres. */
      @JavascriptInterface
      fun setIconTheme(theme: String) {
        val themes = listOf("jaune", "rouge", "vert", "violet")
        if (theme !in themes) return
        // La nouvelle entrée d'abord, pour que le lanceur ne se retrouve jamais sans Maku.
        for (name in themes.sortedBy { it != theme }) {
          val component = ComponentName(this@MainActivity, "$packageName.Icon${name.replaceFirstChar { it.uppercase() }}")
          val wanted = if (name == theme) PackageManager.COMPONENT_ENABLED_STATE_ENABLED else PackageManager.COMPONENT_ENABLED_STATE_DISABLED
          val current = when (val state = packageManager.getComponentEnabledSetting(component)) {
            PackageManager.COMPONENT_ENABLED_STATE_DEFAULT ->
              if (name == "jaune") PackageManager.COMPONENT_ENABLED_STATE_ENABLED else PackageManager.COMPONENT_ENABLED_STATE_DISABLED
            else -> state
          }
          if (current != wanted) packageManager.setComponentEnabledSetting(component, wanted, PackageManager.DONT_KILL_APP)
        }
      }

      /**
       * Mise à jour : confie l'APK téléchargé (dans le cache de Maku) à l'installateur d'Android.
       * Renvoie « permission » si Maku n'a pas encore le droit d'installer des applications : Android
       * ouvre alors l'écran où l'accorder, et il suffit de relancer la mise à jour.
       */
      @JavascriptInterface
      fun installApk(path: String): String {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && !packageManager.canRequestPackageInstalls()) {
          runOnUiThread {
            try {
              startActivity(Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:$packageName")))
            } catch (_: Exception) {
              // Certains téléviseurs n'ont pas cet écran : l'autorisation se donne dans leurs paramètres.
            }
          }
          return "permission"
        }
        val uri = FileProvider.getUriForFile(this@MainActivity, "$packageName.fileprovider", File(path))
        runOnUiThread {
          startActivity(
            Intent(Intent.ACTION_VIEW)
              .setDataAndType(uri, "application/vnd.android.package-archive")
              .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK),
          )
        }
        return "ok"
      }
    }, "PlexrDevice")
  }
}
