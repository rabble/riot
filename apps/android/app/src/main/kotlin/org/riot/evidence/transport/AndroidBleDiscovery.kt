package org.riot.evidence.transport

import android.annotation.SuppressLint
import android.bluetooth.BluetoothManager
import android.bluetooth.le.AdvertiseCallback
import android.bluetooth.le.AdvertiseData
import android.bluetooth.le.AdvertiseSettings
import android.bluetooth.le.ScanCallback
import android.bluetooth.le.ScanFilter
import android.bluetooth.le.ScanResult
import android.bluetooth.le.ScanSettings
import android.content.Context
import android.os.ParcelUuid
import java.nio.charset.StandardCharsets
import java.util.UUID

class AndroidBleDiscovery(
    context: Context,
    val friendlyName: String = FriendlyNameGenerator.generate(),
) : AutoCloseable {
    private val adapter = context.getSystemService(BluetoothManager::class.java)?.adapter
        ?: throw IllegalStateException("Nearby Bluetooth is unavailable")
    private val service = ParcelUuid(SERVICE_UUID)
    private val seenHandles = mutableSetOf<String>()
    private var scanCallback: ScanCallback? = null
    private var advertiseCallback: AdvertiseCallback? = null

    @SuppressLint("MissingPermission")
    fun start(onPhoneFound: (DiscoveredPhone) -> Unit, onFailure: () -> Unit) {
        close()
        if (!adapter.isEnabled) throw IllegalStateException("Nearby Bluetooth is turned off")
        val advertiser = adapter.bluetoothLeAdvertiser
            ?: throw IllegalStateException("Nearby advertising is unavailable")
        val scanner = adapter.bluetoothLeScanner
            ?: throw IllegalStateException("Nearby scanning is unavailable")
        advertiseCallback = object : AdvertiseCallback() {
            override fun onStartFailure(errorCode: Int) = onFailure()
        }.also { callback ->
            advertiser.startAdvertising(
                AdvertiseSettings.Builder()
                    .setAdvertiseMode(AdvertiseSettings.ADVERTISE_MODE_LOW_LATENCY)
                    .setConnectable(true)
                    .setTimeout(0)
                    .build(),
                AdvertiseData.Builder()
                    .addServiceUuid(service)
                    .addServiceData(service, FriendlyNameAdvertisement.encode(friendlyName))
                    .setIncludeDeviceName(false)
                    .build(),
                callback,
            )
        }
        scanCallback = object : ScanCallback() {
            override fun onScanResult(callbackType: Int, result: ScanResult) {
                val name = result.scanRecord?.getServiceData(service)
                    ?.let(FriendlyNameAdvertisement::decode) ?: return
                val handle = result.device.address
                if (seenHandles.add(handle)) {
                    onPhoneFound(DiscoveredPhone(name, PeerHandle(handle)))
                }
            }

            override fun onScanFailed(errorCode: Int) = onFailure()
        }.also { callback ->
            scanner.startScan(
                listOf(ScanFilter.Builder().setServiceUuid(service).build()),
                ScanSettings.Builder()
                    .setScanMode(ScanSettings.SCAN_MODE_LOW_LATENCY)
                    .build(),
                callback,
            )
        }
    }

    @SuppressLint("MissingPermission")
    override fun close() {
        scanCallback?.let { adapter.bluetoothLeScanner?.stopScan(it) }
        advertiseCallback?.let { adapter.bluetoothLeAdvertiser?.stopAdvertising(it) }
        scanCallback = null
        advertiseCallback = null
        seenHandles.clear()
    }

    companion object {
        val SERVICE_UUID: UUID = UUID.fromString("8ddf2f16-92b1-4e4f-a18e-191c23060001")
        val DATA_UUID: UUID = UUID.fromString("8ddf2f16-92b1-4e4f-a18e-191c23060002")
        val ENDPOINT_UUID: UUID = UUID.fromString("8ddf2f16-92b1-4e4f-a18e-191c23060003")
    }
}

object FriendlyNameAdvertisement {
    private const val MAX_NAME_BYTES = 24
    private val SAFE_NAME = Regex("[A-Z][a-z]+ [A-Z][a-z]+")

    fun encode(name: String): ByteArray {
        require(SAFE_NAME.matches(name)) { "invalid friendly name" }
        return name.toByteArray(StandardCharsets.UTF_8).also {
            require(it.size <= MAX_NAME_BYTES) { "friendly name is too long" }
        }
    }

    fun decode(bytes: ByteArray): String? {
        if (bytes.size !in 1..MAX_NAME_BYTES) return null
        val name = bytes.toString(StandardCharsets.UTF_8)
        return name.takeIf(SAFE_NAME::matches)
    }
}
