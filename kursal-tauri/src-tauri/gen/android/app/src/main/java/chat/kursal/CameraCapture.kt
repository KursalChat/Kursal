package chat.kursal

import android.content.Context
import android.graphics.ImageFormat
import android.hardware.camera2.CameraCaptureSession
import android.hardware.camera2.CameraCharacteristics
import android.hardware.camera2.CameraDevice
import android.hardware.camera2.CameraManager
import android.hardware.camera2.CaptureRequest
import android.hardware.camera2.params.OutputConfiguration
import android.hardware.camera2.params.SessionConfiguration
import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.util.Log
import android.view.Surface
import java.nio.ByteBuffer
import java.util.concurrent.Executor

/** Camera2 frames go straight into a MediaCodec input Surface. */
object CameraCapture {
    private const val TAG = "KursalCamera"

    private var camera: CameraDevice? = null
    private var session: CameraCaptureSession? = null
    private var encoder: MediaCodec? = null
    private var inputSurface: Surface? = null
    private var thread: HandlerThread? = null
    private var handler: Handler? = null
    private var drain: Thread? = null
    private var codecConfig: ByteArray? = null

    @Volatile private var running = false

    // Surface input arrives in sensor orientation and cannot be rotated without
    // a GL pass, so the angle travels with each frame and the renderer applies it.
    @Volatile private var sensorOrientation = 0
    @Volatile private var frontFacing = false
    @Volatile private var deviceAngle = 0
    @Volatile private var frameRotation = 0

    private fun recomputeRotation() {
        val device = deviceAngle
        frameRotation = if (frontFacing) {
            (sensorOrientation + device) % 360
        } else {
            (sensorOrientation - device + 360) % 360
        }
    }

    @JvmStatic
    @Synchronized
    fun setDeviceAngle(angle: Int) {
        deviceAngle = ((angle % 360) + 360) % 360
        recomputeRotation()
    }

    @JvmStatic
    external fun nativeVideoFrame(
        data: ByteArray,
        keyframe: Boolean,
        timestampUs: Long,
        rotation: Int,
    )

    @JvmStatic
    external fun nativeCaptureFailed()

    /** Only for deaths after a successful start */
    private fun reportFailure(reason: String) {
        if (!running) return
        running = false
        Log.e(TAG, "capture failed: $reason")
        try {
            nativeCaptureFailed()
        } catch (e: Throwable) {
            Log.e(TAG, "native failure report failed", e)
        }
    }

    @JvmStatic
    fun listCameras(context: Context): Array<String> {
        val manager = context.getSystemService(Context.CAMERA_SERVICE) as CameraManager
        return try {
            // No label: Camera2 has none localised, so the UI builds one from the facing.
            manager.cameraIdList.mapNotNull { id ->
                val chars = manager.getCameraCharacteristics(id)
                val facing = when (chars.get(CameraCharacteristics.LENS_FACING)) {
                    CameraCharacteristics.LENS_FACING_FRONT -> "user"
                    CameraCharacteristics.LENS_FACING_BACK -> "environment"
                    else -> ""
                }
                "$id\t\t$facing"
            }.toTypedArray()
        } catch (e: Throwable) {
            Log.e(TAG, "listCameras failed", e)
            emptyArray()
        }
    }

    /** Returns "width|height|cameraId", or null when the camera could not start. */
    @JvmStatic
    @Synchronized
    fun start(
        context: Context,
        requestedId: String?,
        width: Int,
        height: Int,
        bitrate: Int,
        keyIntervalSecs: Int,
    ): String? {
        stop(context)
        val manager = context.getSystemService(Context.CAMERA_SERVICE) as CameraManager
        return try {
            val cameraId = requestedId?.takeIf { manager.cameraIdList.contains(it) }
                ?: manager.cameraIdList.firstOrNull {
                    manager.getCameraCharacteristics(it)
                        .get(CameraCharacteristics.LENS_FACING) ==
                        CameraCharacteristics.LENS_FACING_FRONT
                }
                ?: manager.cameraIdList.firstOrNull()
                ?: return null

            val chars = manager.getCameraCharacteristics(cameraId)
            sensorOrientation = chars.get(CameraCharacteristics.SENSOR_ORIENTATION) ?: 0
            frontFacing = chars.get(CameraCharacteristics.LENS_FACING) ==
                CameraCharacteristics.LENS_FACING_FRONT
            recomputeRotation()

            val size = chooseSize(manager, cameraId, width, height)
            startEncoder(size.first, size.second, bitrate, keyIntervalSecs)

            val ht = HandlerThread("kursal-camera").apply { start() }
            thread = ht
            handler = Handler(ht.looper)

            openCamera(manager, cameraId)
            running = true
            startDrain()
            "${size.first}|${size.second}|$cameraId"
        } catch (e: Throwable) {
            Log.e(TAG, "camera start failed", e)
            stop(context)
            null
        }
    }

    private fun chooseSize(
        manager: CameraManager,
        cameraId: String,
        width: Int,
        height: Int,
    ): Pair<Int, Int> {
        val map = manager.getCameraCharacteristics(cameraId)
            .get(CameraCharacteristics.SCALER_STREAM_CONFIGURATION_MAP)
            ?: return Pair(width, height)
        val sizes = map.getOutputSizes(MediaCodec::class.java) ?: return Pair(width, height)
        val wantedArea = width.toDouble() * height.toDouble()
        val wantedAr = width.toDouble() / height.toDouble()
        // Area alone picks 4:3 for a 16:9 budget, letterboxed for the whole call.
        val best = sizes
            .filter { it.width >= it.height }
            .minByOrNull {
                val ar = it.width.toDouble() / it.height.toDouble()
                val area = it.width.toDouble() * it.height.toDouble()
                Math.abs(ar - wantedAr) * 4.0 + Math.abs(area - wantedArea) / wantedArea
            }
            ?: return Pair(width, height)
        return Pair(best.width, best.height)
    }

    private fun startEncoder(width: Int, height: Int, bitrate: Int, keyIntervalSecs: Int) {
        val format = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, width, height)
        format.setInteger(
            MediaFormat.KEY_COLOR_FORMAT,
            MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface,
        )
        format.setInteger(MediaFormat.KEY_BIT_RATE, bitrate)
        format.setInteger(MediaFormat.KEY_FRAME_RATE, 30)
        format.setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, keyIntervalSecs)
        format.setInteger(
            MediaFormat.KEY_PROFILE,
            MediaCodecInfo.CodecProfileLevel.AVCProfileBaseline,
        )
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            format.setInteger(
                MediaFormat.KEY_BITRATE_MODE,
                MediaCodecInfo.EncoderCapabilities.BITRATE_MODE_CBR,
            )
        }

        val codec = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_VIDEO_AVC)
        codec.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
        inputSurface = codec.createInputSurface()
        codec.start()
        encoder = codec
    }

    private fun openCamera(manager: CameraManager, cameraId: String) {
        val surface = inputSurface ?: throw IllegalStateException("no encoder surface")
        val latch = java.util.concurrent.CountDownLatch(1)
        var failure: Throwable? = null

        manager.openCamera(cameraId, object : CameraDevice.StateCallback() {
            override fun onOpened(device: CameraDevice) {
                camera = device
                try {
                    configureSession(device, surface) { err ->
                        failure = err
                        latch.countDown()
                    }
                } catch (e: Throwable) {
                    failure = e
                    latch.countDown()
                }
            }

            override fun onDisconnected(device: CameraDevice) {
                failure = IllegalStateException("camera disconnected")
                device.close()
                camera = null
                latch.countDown()
                reportFailure("camera disconnected")
            }

            override fun onError(device: CameraDevice, error: Int) {
                failure = IllegalStateException("camera error $error")
                device.close()
                camera = null
                latch.countDown()
                reportFailure("camera error $error")
            }
        }, handler)

        if (!latch.await(5, java.util.concurrent.TimeUnit.SECONDS)) {
            throw IllegalStateException("camera open timed out")
        }
        failure?.let { throw it }
    }

    private fun configureSession(
        device: CameraDevice,
        surface: Surface,
        done: (Throwable?) -> Unit,
    ) {
        val request = device.createCaptureRequest(CameraDevice.TEMPLATE_RECORD).apply {
            addTarget(surface)
            set(CaptureRequest.CONTROL_AE_MODE, CaptureRequest.CONTROL_AE_MODE_ON)
        }

        val callback = object : CameraCaptureSession.StateCallback() {
            override fun onConfigured(configured: CameraCaptureSession) {
                session = configured
                var err: Throwable? = null
                try {
                    configured.setRepeatingRequest(request.build(), null, handler)
                } catch (e: Throwable) {
                    err = e
                }
                done(err)
            }

            override fun onConfigureFailed(configured: CameraCaptureSession) {
                done(IllegalStateException("capture session configuration failed"))
            }
        }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            val executor = Executor { command -> handler?.post(command) }
            device.createCaptureSession(
                SessionConfiguration(
                    SessionConfiguration.SESSION_REGULAR,
                    listOf(OutputConfiguration(surface)),
                    executor,
                    callback,
                )
            )
        } else {
            @Suppress("DEPRECATION")
            device.createCaptureSession(listOf(surface), callback, handler)
        }
    }

    private fun startDrain() {
        val codec = encoder ?: return
        drain = Thread {
            val info = MediaCodec.BufferInfo()
            var reason: String? = null
            while (running) {
                val index = try {
                    codec.dequeueOutputBuffer(info, 10_000)
                } catch (e: Throwable) {
                    if (running) Log.e(TAG, "dequeue failed", e)
                    reason = "encoder dequeue failed"
                    break
                }
                if (index < 0) continue

                val buffer: ByteBuffer? = try {
                    codec.getOutputBuffer(index)
                } catch (e: Throwable) {
                    null
                }
                if (buffer != null && info.size > 0) {
                    buffer.position(info.offset)
                    buffer.limit(info.offset + info.size)
                    val bytes = ByteArray(info.size)
                    buffer.get(bytes)

                    if (info.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG != 0) {
                        // SPS/PPS arrive once, and every later keyframe needs them prepended.
                        codecConfig = bytes
                    } else {
                        val keyframe = info.flags and MediaCodec.BUFFER_FLAG_KEY_FRAME != 0
                        val payload = if (keyframe) {
                            val csd = codecConfig
                            if (csd != null) csd + bytes else bytes
                        } else {
                            bytes
                        }
                        try {
                            nativeVideoFrame(
                                payload,
                                keyframe,
                                info.presentationTimeUs,
                                frameRotation,
                            )
                        } catch (e: Throwable) {
                            Log.e(TAG, "native frame delivery failed", e)
                        }
                    }
                }

                try {
                    codec.releaseOutputBuffer(index, false)
                } catch (e: Throwable) {
                    reason = "encoder release failed"
                    break
                }
            }
            reason?.let { reportFailure(it) }
        }.also { it.name = "kursal-video-drain"; it.start() }
    }

    @JvmStatic
    @Synchronized
    fun requestKeyframe() {
        val codec = encoder ?: return
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.KITKAT) return
        try {
            val params = android.os.Bundle()
            params.putInt(MediaCodec.PARAMETER_KEY_REQUEST_SYNC_FRAME, 0)
            codec.setParameters(params)
        } catch (e: Throwable) {
            Log.e(TAG, "keyframe request failed", e)
        }
    }

    @JvmStatic
    @Synchronized
    fun setBitrate(bps: Int) {
        val codec = encoder ?: return
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.KITKAT) return
        try {
            val params = android.os.Bundle()
            params.putInt(MediaCodec.PARAMETER_KEY_VIDEO_BITRATE, bps)
            codec.setParameters(params)
        } catch (e: Throwable) {
            Log.e(TAG, "bitrate change failed", e)
        }
    }

    @JvmStatic
    @Synchronized
    fun stop(context: Context) {
        running = false
        try { session?.stopRepeating() } catch (_: Throwable) {}
        try { session?.close() } catch (_: Throwable) {}
        session = null
        try { camera?.close() } catch (_: Throwable) {}
        camera = null

        // Releasing the codec while the drain thread is inside it aborts natively. Leak instead.
        val stuck = drain?.let { worker ->
            try { worker.join(3000) } catch (_: Throwable) {}
            worker.isAlive
        } ?: false
        drain = null

        if (stuck) {
            Log.w(TAG, "drain thread still running; leaking the encoder")
        } else {
            try { encoder?.stop() } catch (_: Throwable) {}
            try { encoder?.release() } catch (_: Throwable) {}
            try { inputSurface?.release() } catch (_: Throwable) {}
        }
        encoder = null
        inputSurface = null
        codecConfig = null

        thread?.quitSafely()
        thread = null
        handler = null
    }
}
