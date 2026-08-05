package chat.kursal

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.view.WindowManager
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  private val startupPermsRequestCode = 4242
  private val insets = InsetsBridge()
  private val bars = BarsBridge()
  private var webView: WebView? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    io.crates.keyring.Keyring.initializeNdkContext(applicationContext)
    super.onCreate(savedInstanceState)
    goEdgeToEdge()
    requestStartupPermissions()
    startConnectionService()
    takeShareIntent(intent)
  }

  // Android 15+ forces edge-to-edge and ignores all of this. Below it the
  // window stops at the system bars instead, so the app renders inside opaque
  // status/navigation bands and the inset listener only ever reports zeroes.
  @Suppress("DEPRECATION")
  private fun goEdgeToEdge() {
    WindowCompat.setDecorFitsSystemWindows(window, false)
    window.statusBarColor = Color.TRANSPARENT
    window.navigationBarColor = Color.TRANSPARENT

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
      window.isStatusBarContrastEnforced = false
      window.isNavigationBarContrastEnforced = false
    }

    // ALWAYS matches what 15+ defaults to, so a side cutout in landscape is
    // drawn into rather than letterboxed. The web layer pads around it.
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
      val mode =
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
          WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
        } else {
          WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
      window.attributes = window.attributes.apply { layoutInDisplayCutoutMode = mode }
    }
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    takeShareIntent(intent)
  }

  private fun takeShareIntent(intent: Intent?) {
    ShareIntake.handle(this, intent) {
      runOnUiThread {
        webView?.evaluateJavascript("window.__kursalOnShare && window.__kursalOnShare()", null)
      }
    }
  }

  override fun onWebViewCreate(webView: WebView) {
    this.webView = webView
    webView.addJavascriptInterface(insets, "__kursalInsets")
    webView.addJavascriptInterface(bars, "__kursalBars")

    ViewCompat.setOnApplyWindowInsetsListener(webView) { view, windowInsets ->
      val barInsets = windowInsets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout(),
      )
      val ime = windowInsets.getInsets(WindowInsetsCompat.Type.ime())
      val density = view.resources.displayMetrics.density
      insets.update(
        barInsets.left / density,
        barInsets.top / density,
        barInsets.right / density,
        barInsets.bottom / density,
        ime.bottom / density,
      )
      webView.evaluateJavascript(
        "window.__kursalOnInsets && window.__kursalOnInsets(${insets.read()})",
        null,
      )
      windowInsets
    }
    ViewCompat.requestApplyInsets(webView)
  }

  inner class BarsBridge {
    @JavascriptInterface
    fun setLightBackground(light: Boolean) {
      runOnUiThread {
        WindowCompat.getInsetsController(window, window.decorView).apply {
          isAppearanceLightStatusBars = light
          isAppearanceLightNavigationBars = light
        }
      }
    }
  }

  class InsetsBridge {
    @Volatile private var left = 0f
    @Volatile private var top = 0f
    @Volatile private var right = 0f
    @Volatile private var bottom = 0f
    @Volatile private var ime = 0f

    fun update(l: Float, t: Float, r: Float, b: Float, imeBottom: Float) {
      left = l
      top = t
      right = r
      bottom = b
      ime = imeBottom
    }

    @JavascriptInterface
    fun read(): String = "$top,$right,$bottom,$left,$ime"
  }

  // Android only allows one permission request in flight: issuing separate
  // requestPermissions() calls back to back silently drops all but the first,
  // which is how RECORD_AUDIO ended up never being asked for.
  private fun requestStartupPermissions() {
    val wanted = mutableListOf<String>()

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      wanted.add(Manifest.permission.BLUETOOTH_ADVERTISE)
      wanted.add(Manifest.permission.BLUETOOTH_CONNECT)
      wanted.add(Manifest.permission.BLUETOOTH_SCAN)
    } else {
      wanted.add(Manifest.permission.ACCESS_FINE_LOCATION)
    }

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
      wanted.add(Manifest.permission.POST_NOTIFICATIONS)
    }

    wanted.add(Manifest.permission.RECORD_AUDIO)

    val missing = wanted.filter {
      ContextCompat.checkSelfPermission(this, it) != PackageManager.PERMISSION_GRANTED
    }
    if (missing.isNotEmpty()) {
      ActivityCompat.requestPermissions(this, missing.toTypedArray(), startupPermsRequestCode)
    }
  }

  private fun startConnectionService() {
    val intent = Intent(this, P2pForegroundService::class.java)
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      ContextCompat.startForegroundService(this, intent)
    } else {
      startService(intent)
    }
  }
}
