package chat.kursal

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattServer
import android.bluetooth.BluetoothGattServerCallback
import android.bluetooth.BluetoothGattService
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.bluetooth.le.BluetoothLeAdvertiser
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.ParcelUuid
import android.util.Log
import androidx.core.content.ContextCompat
import java.util.UUID

@SuppressLint("MissingPermission")
object BleAdvertiser {
  private const val TAG = "BleAdvertiser"

  private var gattServer: BluetoothGattServer? = null
  private var advertiser: BluetoothLeAdvertiser? = null
  private var advCallback: AdvertiseCallback? = null
  private var serviceUuid: UUID? = null
  private var charUuid: UUID? = null

  external fun nativeOnReadRequest(): ByteArray
  external fun nativeOnWriteRequest(client: String, data: ByteArray)

  @JvmStatic
  fun start(ctx: Context, serviceUuidStr: String, charUuidStr: String, name: String): Boolean {
    if (gattServer != null) {
      Log.i(TAG, "already started")
      return true
    }
    if (!hasPerms(ctx)) {
      Log.w(TAG, "missing BLE permissions, cannot advertise")
      return false
    }

    val mgr = ctx.getSystemService(Context.BLUETOOTH_SERVICE) as? BluetoothManager
    if (mgr == null) {
      Log.e(TAG, "no BluetoothManager")
      return false
    }
    val adapter = mgr.adapter
    if (adapter == null || !adapter.isEnabled) {
      Log.w(TAG, "bluetooth disabled")
      return false
    }
    if (!adapter.isMultipleAdvertisementSupported) {
      Log.w(TAG, "device does not support BLE advertising")
      return false
    }

    val advr = adapter.bluetoothLeAdvertiser
    if (advr == null) {
      Log.w(TAG, "no BluetoothLeAdvertiser")
      return false
    }

    val sUuid = UUID.fromString(serviceUuidStr)
    val cUuid = UUID.fromString(charUuidStr)
    serviceUuid = sUuid
    charUuid = cUuid

    val server = mgr.openGattServer(ctx, gattCallback)
    if (server == null) {
      Log.e(TAG, "openGattServer failed")
      return false
    }
    val service = BluetoothGattService(sUuid, BluetoothGattService.SERVICE_TYPE_PRIMARY)
    val ch = BluetoothGattCharacteristic(
      cUuid,
      BluetoothGattCharacteristic.PROPERTY_READ or BluetoothGattCharacteristic.PROPERTY_WRITE,
      BluetoothGattCharacteristic.PERMISSION_READ or BluetoothGattCharacteristic.PERMISSION_WRITE,
    )
    service.addCharacteristic(ch)
    server.addService(service)
    gattServer = server

    val settings = AdvertiseSettings.Builder()
      .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_BALANCED)
      .setTxPowerLevel(AdvertiseSettings.ADVERTISE_TX_POWER_MEDIUM)
      .setConnectable(true)
      .setTimeout(0)
      .build()

    val data = AdvertiseData.Builder()
      .setIncludeDeviceName(false)
      .setIncludeTxPowerLevel(false)
      .addServiceUuid(ParcelUuid(sUuid))
      .build()

    val cb = object : AdvertiseCallback() {
      override fun onStartSuccess(settingsInEffect: AdvertiseSettings) {
        Log.i(TAG, "advertising started ($name)")
      }
      override fun onStartFailure(errorCode: Int) {
        Log.e(TAG, "advertising start failed: $errorCode")
      }
    }

    advertiser = advr
    advCallback = cb
    advr.startAdvertising(settings, data, cb)
    return true
  }

  @JvmStatic
  fun stop(ctx: Context) {
    val cb = advCallback
    val advr = advertiser
    if (cb != null && advr != null && hasPerms(ctx)) {
      try {
        advr.stopAdvertising(cb)
      } catch (e: Exception) {
        Log.w(TAG, "stop advertising: $e")
      }
    }
    advCallback = null
    advertiser = null

    val srv = gattServer
    if (srv != null && hasPerms(ctx)) {
      try {
        srv.close()
      } catch (e: Exception) {
        Log.w(TAG, "close server: $e")
      }
    }
    gattServer = null
    serviceUuid = null
    charUuid = null
    Log.i(TAG, "stopped")
  }

  private fun hasPerms(ctx: Context): Boolean {
    return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
      ContextCompat.checkSelfPermission(ctx, Manifest.permission.BLUETOOTH_ADVERTISE) ==
        PackageManager.PERMISSION_GRANTED &&
        ContextCompat.checkSelfPermission(ctx, Manifest.permission.BLUETOOTH_CONNECT) ==
        PackageManager.PERMISSION_GRANTED
    } else {
      true
    }
  }

  private val gattCallback = object : BluetoothGattServerCallback() {
    override fun onConnectionStateChange(device: BluetoothDevice, status: Int, newState: Int) {
      Log.i(TAG, "conn state ${device.address}: $newState (status $status)")
    }

    override fun onCharacteristicReadRequest(
      device: BluetoothDevice,
      requestId: Int,
      offset: Int,
      characteristic: BluetoothGattCharacteristic,
    ) {
      val srv = gattServer ?: return
      if (characteristic.uuid != charUuid) {
        srv.sendResponse(device, requestId, BluetoothGatt.GATT_FAILURE, 0, null)
        return
      }
      val full = try { nativeOnReadRequest() } catch (e: Throwable) {
        Log.e(TAG, "nativeOnReadRequest: $e"); ByteArray(0)
      }
      val slice = if (offset >= full.size) ByteArray(0)
        else full.copyOfRange(offset, full.size)
      srv.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, offset, slice)
    }

    override fun onCharacteristicWriteRequest(
      device: BluetoothDevice,
      requestId: Int,
      characteristic: BluetoothGattCharacteristic,
      preparedWrite: Boolean,
      responseNeeded: Boolean,
      offset: Int,
      value: ByteArray,
    ) {
      val srv = gattServer
      if (characteristic.uuid != charUuid) {
        if (responseNeeded && srv != null) {
          srv.sendResponse(device, requestId, BluetoothGatt.GATT_FAILURE, 0, null)
        }
        return
      }
      try {
        nativeOnWriteRequest(device.address, value)
      } catch (e: Throwable) {
        Log.e(TAG, "nativeOnWriteRequest: $e")
      }
      if (responseNeeded && srv != null) {
        srv.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, offset, null)
      }
    }

    override fun onMtuChanged(device: BluetoothDevice, mtu: Int) {
      Log.i(TAG, "mtu changed ${device.address}: $mtu")
    }
  }
}
