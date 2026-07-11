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
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.security.SecureRandom
import java.util.UUID

class AndroidBleDiscovery(
    context: Context,
    val friendlyName: String = FriendlyNameGenerator.generate(),
) : AutoCloseable {
    private val secureRandom = SecureRandom()
    var pairingToken: Int = secureRandom.nextInt(65_536)
        private set
    private val adapter = context.getSystemService(BluetoothManager::class.java)?.adapter
        ?: throw IllegalStateException("Nearby Bluetooth is unavailable")
    private val service = ParcelUuid(SERVICE_UUID)
    private val seenHandles = mutableSetOf<String>()
    private var scanCallback: ScanCallback? = null
    private var advertiseCallback: AdvertiseCallback? = null

    @SuppressLint("MissingPermission")
    fun start(onPhoneFound: (DiscoveredPhone) -> Unit, onFailure: () -> Unit) {
        close()
        pairingToken = secureRandom.nextInt(65_536)
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
                    .setIncludeDeviceName(false)
                    .build(),
                AdvertiseData.Builder()
                    .addServiceData(
                        service,
                        FriendlyNameAdvertisement.encode(friendlyName, pairingToken),
                    )
                    .build(),
                callback,
            )
        }
        scanCallback = object : ScanCallback() {
            override fun onScanResult(callbackType: Int, result: ScanResult) {
                val advertised = result.scanRecord?.getServiceData(service)
                    ?.let(FriendlyNameAdvertisement::decode) ?: return
                val handle = result.device.address
                if (seenHandles.add(handle)) {
                    onPhoneFound(
                        DiscoveredPhone(advertised.friendlyName, PeerHandle(handle), advertised.pairingToken),
                    )
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
    private const val MAX_NAME_BYTES = 11
    private val SAFE_NAME = Regex("[A-Z][a-z]+ [A-Z][a-z]+")

    fun encode(name: String, pairingToken: Int): ByteArray {
        require(SAFE_NAME.matches(name)) { "invalid friendly name" }
        require(pairingToken in 0..65_535) { "invalid pairing token" }
        val encodedName = name.toByteArray(StandardCharsets.UTF_8)
        require(encodedName.size <= MAX_NAME_BYTES) { "friendly name is too long" }
        return ByteBuffer.allocate(2 + encodedName.size)
            .order(ByteOrder.BIG_ENDIAN)
            .putShort(pairingToken.toShort())
            .put(encodedName)
            .array()
    }

    fun decode(bytes: ByteArray): AdvertisedPhone? {
        if (bytes.size !in 3..(MAX_NAME_BYTES + 2)) return null
        val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN)
        val token = buffer.short.toInt() and 0xffff
        val encodedName = ByteArray(buffer.remaining()).also(buffer::get)
        val name = encodedName.toString(StandardCharsets.UTF_8)
        return name.takeIf(SAFE_NAME::matches)?.let { AdvertisedPhone(it, token) }
    }
}

data class AdvertisedPhone(val friendlyName: String, val pairingToken: Int)
