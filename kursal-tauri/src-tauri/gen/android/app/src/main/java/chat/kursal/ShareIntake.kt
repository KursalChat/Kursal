package chat.kursal

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import android.util.Log
import androidx.core.content.IntentCompat
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.UUID
import java.util.concurrent.Executors

// Copies shared items into cacheDir/kursal-shares/<uuid>/ and writes manifest.json
// last, via a rename, so take_pending_shares never sees a half-copied payload.
object ShareIntake {
  private const val TAG = "ShareIntake"
  private const val SHARE_DIR = "kursal-shares"
  private const val MANIFEST = "manifest.json"

  private val executor = Executors.newSingleThreadExecutor()

  fun handle(context: Context, intent: Intent?, onStaged: () -> Unit) {
    if (intent == null) return
    if (intent.action != Intent.ACTION_SEND && intent.action != Intent.ACTION_SEND_MULTIPLE) return

    val uris = readUris(intent)
    val text = intent.getStringExtra(Intent.EXTRA_TEXT)
    if (uris.isEmpty() && text.isNullOrBlank()) return

    // Consumed here so a relaunch or config change can't import the same share twice.
    intent.action = Intent.ACTION_MAIN
    intent.removeExtra(Intent.EXTRA_STREAM)
    intent.removeExtra(Intent.EXTRA_TEXT)

    val appContext = context.applicationContext
    executor.execute {
      // Copying can outlast the webview's own startup drain, so the frontend is
      // told once the payload is actually complete on disk.
      if (stage(appContext, uris, text)) onStaged()
    }
  }

  private fun readUris(intent: Intent): List<Uri> =
    if (intent.action == Intent.ACTION_SEND_MULTIPLE) {
      IntentCompat.getParcelableArrayListExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
        ?: emptyList()
    } else {
      listOfNotNull(IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java))
    }

  private fun stage(context: Context, uris: List<Uri>, text: String?): Boolean {
    val dir = File(context.cacheDir, "$SHARE_DIR/${UUID.randomUUID()}")
    if (!dir.mkdirs()) {
      Log.w(TAG, "Could not create share staging dir")
      return false
    }

    val files = JSONArray()
    uris.forEachIndexed { index, uri ->
      val copied = copyItem(context, uri, dir, index)
      if (copied != null) files.put(copied)
    }

    if (files.length() == 0 && text.isNullOrBlank()) {
      dir.deleteRecursively()
      return false
    }

    val manifest = JSONObject().apply {
      put("files", files)
      if (!text.isNullOrBlank()) put("text", text)
    }

    val temp = File(dir, "$MANIFEST.tmp")
    return try {
      temp.writeText(manifest.toString())
      if (!temp.renameTo(File(dir, MANIFEST))) throw java.io.IOException("manifest rename failed")
      true
    } catch (err: Exception) {
      Log.w(TAG, "Failed to write share manifest", err)
      dir.deleteRecursively()
      false
    }
  }

  private fun copyItem(context: Context, uri: Uri, dir: File, index: Int): JSONObject? {
    val name = displayName(context, uri) ?: fallbackName(context, uri, index)
    val target = File(dir, name)

    return try {
      context.contentResolver.openInputStream(uri)?.use { input ->
        target.outputStream().use { output -> input.copyTo(output) }
      } ?: return null

      JSONObject().apply {
        put("path", target.absolutePath)
        put("filename", name)
        put("sizeBytes", target.length())
      }
    } catch (err: Exception) {
      Log.w(TAG, "Failed to copy shared item $uri", err)
      target.delete()
      null
    }
  }

  private fun displayName(context: Context, uri: Uri): String? {
    if (uri.scheme == "file") return uri.lastPathSegment?.let(::sanitize)

    return try {
      context.contentResolver
        .query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
        ?.use { cursor ->
          val column = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
          if (column >= 0 && cursor.moveToFirst()) {
            cursor.getString(column)?.takeIf { it.isNotBlank() }?.let(::sanitize)
          } else {
            null
          }
        }
    } catch (err: Exception) {
      Log.w(TAG, "Could not read display name for $uri", err)
      null
    }
  }

  private fun fallbackName(context: Context, uri: Uri, index: Int): String {
    val extension = context.contentResolver.getType(uri)
      ?.let { android.webkit.MimeTypeMap.getSingleton().getExtensionFromMimeType(it) }
      ?: "bin"
    return "shared-${index + 1}.$extension"
  }

  private fun sanitize(name: String): String {
    val safe = name.trim().replace(Regex("[^a-zA-Z0-9._-]"), "_")
    return safe.ifEmpty { "file" }
  }
}
