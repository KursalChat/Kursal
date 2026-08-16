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
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  private val insets = InsetsBridge()
  private val bars = BarsBridge()
  private val perms = PermsBridge()
  private var webView: WebView? = null

  private class PermRequest(val token: String, val permissions: Array<String>)

  private val permQueue = ArrayDeque<PermRequest>()
  private var permInFlight: PermRequest? = null
  private var nextPermToken = 0

  private val permLauncher =
    registerForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { result ->
      val done = permInFlight
      permInFlight = null
      if (done != null) {
        val granted = result.isNotEmpty() && result.values.all { it }
        if (granted && done.permissions.contains(Manifest.permission.BLUETOOTH_ADVERTISE)) {
          startConnectionService()
        }
        replyPermission(done.token, granted)
      }
      launchNextPermission()
    }

  override fun onCreate(savedInstanceState: Bundle?) {
    io.crates.keyring.Keyring.initializeNdkContext(applicationContext)
    super.onCreate(savedInstanceState)
    goEdgeToEdge()
    startConnectionService()
    takeShareIntent(intent)
  }

  // Android 15+ forces edge-to-edge and ignores all of this
  @Suppress("DEPRECATION")
  private fun goEdgeToEdge() {
    WindowCompat.setDecorFitsSystemWindows(window, false)
    window.statusBarColor = Color.TRANSPARENT
    window.navigationBarColor = Color.TRANSPARENT

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
      window.isStatusBarContrastEnforced = false
      window.isNavigationBarContrastEnforced = false
    }

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
    webView.addJavascriptInterface(perms, "__kursalPerms")

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

  private fun permissionsFor(group: String): Array<String> =
    when (group) {
      "bluetooth" ->
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
          arrayOf(
            Manifest.permission.BLUETOOTH_ADVERTISE,
            Manifest.permission.BLUETOOTH_CONNECT,
            Manifest.permission.BLUETOOTH_SCAN,
          )
        } else {
          arrayOf(Manifest.permission.ACCESS_FINE_LOCATION)
        }
      "microphone" -> arrayOf(Manifest.permission.RECORD_AUDIO)
      else -> emptyArray()
    }

  private fun launchNextPermission() {
    if (permInFlight != null) return
    val next = permQueue.removeFirstOrNull() ?: return
    permInFlight = next
    permLauncher.launch(next.permissions)
  }

  private fun replyPermission(token: String, granted: Boolean) {
    runOnUiThread {
      webView?.evaluateJavascript(
        "window.__kursalOnPerms && window.__kursalOnPerms('$token', $granted)",
        null,
      )
    }
  }

  inner class PermsBridge {
    @JavascriptInterface
    fun has(group: String): Boolean =
      permissionsFor(group).all {
        ContextCompat.checkSelfPermission(this@MainActivity, it) ==
          PackageManager.PERMISSION_GRANTED
      }

    @JavascriptInterface
    fun request(group: String): String {
      val token = (nextPermToken++).toString()
      val wanted = permissionsFor(group)
      if (wanted.isEmpty() || has(group)) {
        replyPermission(token, true)
        return token
      }
      runOnUiThread {
        permQueue.addLast(PermRequest(token, wanted))
        launchNextPermission()
      }
      return token
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
