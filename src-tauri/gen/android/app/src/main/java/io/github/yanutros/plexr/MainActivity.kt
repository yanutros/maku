package io.github.yanutros.plexr

import android.app.UiModeManager
import android.content.ComponentName
import android.content.Intent
import android.content.pm.ActivityInfo
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.net.ConnectivityManager
import android.net.Network
import android.net.Uri
import android.net.wifi.WifiManager
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
    DiagLog.init(this)
    // Chromecast : sans ce verrou, le Wi-Fi filtre les annonces mDNS des enceintes (cast.rs les cherche).
    runCatching {
      (applicationContext.getSystemService(WIFI_SERVICE) as WifiManager).createMulticastLock("maku-cast").apply { setReferenceCounted(false); acquire() }
    }
    // Android 17 : sans cette autorisation, les serveurs du réseau local (Jellyfin, Plex en local)
    // sont injoignables.
    if (Build.VERSION.SDK_INT >= 37 && checkSelfPermission(LOCAL_NETWORK) != PackageManager.PERMISSION_GRANTED) {
      requestPermissions(arrayOf(LOCAL_NETWORK), 0)
    }
    // Changement de réseau (Wi-Fi ↔ données mobiles) : la page revérifie l'adresse du serveur. Le premier
    // appel, à l'inscription, ne fait que décrire le réseau du moment.
    val connectivity = getSystemService(CONNECTIVITY_SERVICE) as ConnectivityManager
    connectivity.registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
      private var current: Network? = null
      override fun onAvailable(network: Network) {
        val changed = current != null && current != network
        current = network
        if (changed) notifyPage("__makuNetwork")
      }
    })
  }

  /**
   * Retour dans Maku : la page n'en est pas toujours prévenue (Tauri met le WebView en pause, et la musique
   * le relance aussitôt), on l'appelle donc directement. Elle revérifie l'adresse du serveur.
   */
  override fun onResume() {
    super.onResume()
    DiagLog.log("Maku revient au premier plan")
    notifyPage("__makuResume")
  }

  private fun notifyPage(fn: String) {
    val web = MediaService.webView ?: return
    web.post { web.evaluateJavascript("window.$fn && window.$fn()", null) }
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
    DiagLog.log("Maku passe en arrière-plan")
    video?.command("pause", "")
  }

  /** Android demande de libérer de la mémoire : signe d'un appareil à court, noté dans le journal. */
  override fun onTrimMemory(level: Int) {
    super.onTrimMemory(level)
    DiagLog.log("Android demande de libérer de la mémoire (niveau $level)")
    DiagLog.memory("demande d'Android")
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

      /** Réglages : formats audio que l'ampli, la barre de son ou la TV acceptent tels quels (JSON). */
      @JavascriptInterface
      fun audioPassthrough(): String = NativeVideo.passthroughFormats(this@MainActivity)


      /** Journal de diagnostic (DiagLog.kt) : une ligne de la page, le texte entier, ou son partage. */
      @JavascriptInterface
      fun diagLog(message: String) = DiagLog.log(message)

      @JavascriptInterface
      fun diagText(): String = DiagLog.text()

      @JavascriptInterface
      fun diagShare() {
        val uri = FileProvider.getUriForFile(this@MainActivity, "$packageName.fileprovider", DiagLog.exportFile(this@MainActivity))
        val send = Intent(Intent.ACTION_SEND)
          .setType("text/plain")
          .putExtra(Intent.EXTRA_STREAM, uri)
          .putExtra(Intent.EXTRA_SUBJECT, "Maku : journal de diagnostic")
          .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        runOnUiThread { startActivity(Intent.createChooser(send, null)) }
      }

      /**
       * Invitation : l'image PNG du QR code (en base64), confiée au partage d'Android (Messages, WhatsApp…,
       * ou « Copier »). Le presse-papiers de la page ne sait pas y mettre une image : elle n'arrivait nulle part.
       */
      @JavascriptInterface
      fun shareImage(base64: String, name: String) {
        val file = File(cacheDir, "partage/$name.png")
        file.parentFile?.mkdirs()
        file.writeBytes(android.util.Base64.decode(base64, android.util.Base64.DEFAULT))
        val uri = FileProvider.getUriForFile(this@MainActivity, "$packageName.fileprovider", file)
        val send = Intent(Intent.ACTION_SEND)
          .setType("image/png")
          .putExtra(Intent.EXTRA_STREAM, uri)
          .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        // Aperçu de l'image dans la fenêtre de partage (Android 10 et plus).
        send.clipData = android.content.ClipData.newRawUri(name, uri)
        runOnUiThread { startActivity(Intent.createChooser(send, null)) }
      }

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
