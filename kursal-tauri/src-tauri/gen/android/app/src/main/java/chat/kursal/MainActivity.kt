package chat.kursal

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  private val startupPermsRequestCode = 4242
  private val insets = InsetsBridge()

  override fun onCreate(savedInstanceState: Bundle?) {
    io.crates.keyring.Keyring.initializeNdkContext(applicationContext)
    super.onCreate(savedInstanceState)
    requestStartupPermissions()
    startConnectionService()
  }

  override fun onWebViewCreate(webView: WebView) {
    webView.addJavascriptInterface(insets, "__kursalInsets")

    ViewCompat.setOnApplyWindowInsetsListener(webView) { view, windowInsets ->
      val bars = windowInsets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout(),
      )
      val ime = windowInsets.getInsets(WindowInsetsCompat.Type.ime())
      val density = view.resources.displayMetrics.density
      insets.update(
        bars.left / density,
        bars.top / density,
        bars.right / density,
        bars.bottom / density,
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
