package org.riot.evidence.transport

import android.annotation.SuppressLint
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCallback
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothProfile
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import java.io.IOException
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import java.util.ArrayDeque
import java.util.UUID
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

class AndroidGattBleConnector(private val context: Context) : BleConnector {
    @SuppressLint("MissingPermission")
    override fun connect(handle: PeerHandle): PairedBleLink {
        val adapter = context.getSystemService(BluetoothManager::class.java)?.adapter
            ?: throw IOException("Nearby Bluetooth is unavailable")
        val device = adapter.getRemoteDevice(handle.value)
        val ready = CountDownLatch(1)
        var channel: GattClientFrameChannel? = null
        var endpoint: LocalEndpoint? = null
        var failure: IOException? = null
        val confirmation = RemoteConfirmationWait(MAX_ENDPOINT_READ_ATTEMPTS)
        var endpointCharacteristic: BluetoothGattCharacteristic? = null
        val handler = Handler(Looper.getMainLooper())
        lateinit var requestEndpoint: (BluetoothGatt) -> Unit
        requestEndpoint = { gatt ->
            val characteristic = endpointCharacteristic
            if (characteristic == null) {
                failure = IOException("nearby confirmation unavailable")
                ready.countDown()
            } else if (!confirmation.beginAttempt()) {
                failure = IOException("nearby confirmation timed out")
                ready.countDown()
            } else if (!gatt.readCharacteristic(characteristic)) {
                when (confirmation.completeAttempt(confirmed = false)) {
                    ConfirmationDecision.RETRY -> handler.postDelayed(
                        { requestEndpoint(gatt) },
                        ENDPOINT_RETRY_MILLIS,
                    )
                    ConfirmationDecision.FAILED -> {
                        failure = IOException("nearby confirmation timed out")
                        ready.countDown()
                    }
                    ConfirmationDecision.READY -> Unit
                }
            }
        }

        val callback = object : BluetoothGattCallback() {
            override fun onConnectionStateChange(gatt: BluetoothGatt, status: Int, newState: Int) {
                if (status == BluetoothGatt.GATT_SUCCESS && newState == BluetoothProfile.STATE_CONNECTED) {
                    if (!gatt.discoverServices()) {
                        failure = IOException("service discovery did not start")
                        ready.countDown()
                    }
                } else if (newState == BluetoothProfile.STATE_DISCONNECTED) {
                    channel?.remoteDisconnected()
                    if (channel == null) {
                        failure = IOException("nearby phone disconnected")
                        ready.countDown()
                    }
                }
            }

            override fun onServicesDiscovered(gatt: BluetoothGatt, status: Int) {
                val service = gatt.getService(AndroidBleDiscovery.SERVICE_UUID)
                val data = service?.getCharacteristic(AndroidBleDiscovery.DATA_UUID)
                if (status != BluetoothGatt.GATT_SUCCESS || data == null) {
                    failure = IOException("Riot nearby service unavailable")
                    ready.countDown()
                    return
                }
                channel = GattClientFrameChannel(gatt, data)
                endpointCharacteristic = service.getCharacteristic(AndroidBleDiscovery.ENDPOINT_UUID)
                if (endpointCharacteristic == null) {
                    failure = IOException("nearby confirmation unavailable")
                    ready.countDown()
                    return
                }
                if (!enableNotifications(gatt, data)) {
                    requestEndpoint(gatt)
                }
            }

            override fun onDescriptorWrite(
                gatt: BluetoothGatt,
                descriptor: BluetoothGattDescriptor,
                status: Int,
            ) {
                requestEndpoint(gatt)
            }

            override fun onCharacteristicRead(
                gatt: BluetoothGatt,
                characteristic: BluetoothGattCharacteristic,
                value: ByteArray,
                status: Int,
            ) {
                if (characteristic.uuid != AndroidBleDiscovery.ENDPOINT_UUID) return
                val confirmed = status == BluetoothGatt.GATT_SUCCESS
                when (confirmation.completeAttempt(confirmed)) {
                    ConfirmationDecision.READY -> {
                        endpoint = LocalEndpointAdvertisement.decode(value)
                        ready.countDown()
                    }
                    ConfirmationDecision.RETRY -> handler.postDelayed(
                        { requestEndpoint(gatt) },
                        ENDPOINT_RETRY_MILLIS,
                    )
                    ConfirmationDecision.FAILED -> {
                        failure = IOException("nearby confirmation timed out")
                        ready.countDown()
                    }
                }
            }

            @Deprecated("Used on Android 12 and earlier")
            @Suppress("DEPRECATION")
            override fun onCharacteristicRead(
                gatt: BluetoothGatt,
                characteristic: BluetoothGattCharacteristic,
                status: Int,
            ) = onCharacteristicRead(gatt, characteristic, characteristic.value ?: byteArrayOf(), status)

            override fun onCharacteristicChanged(
                gatt: BluetoothGatt,
                characteristic: BluetoothGattCharacteristic,
                value: ByteArray,
            ) {
                if (characteristic.uuid == AndroidBleDiscovery.DATA_UUID) channel?.receiveChunk(value)
            }

            @Deprecated("Used on Android 12 and earlier")
            @Suppress("DEPRECATION")
            override fun onCharacteristicChanged(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic) =
                onCharacteristicChanged(gatt, characteristic, characteristic.value ?: byteArrayOf())

            override fun onCharacteristicWrite(
                gatt: BluetoothGatt,
                characteristic: BluetoothGattCharacteristic,
                status: Int,
            ) {
                channel?.writeCompleted(status == BluetoothGatt.GATT_SUCCESS)
            }
        }
        val gatt = device.connectGatt(context, false, callback, android.bluetooth.BluetoothDevice.TRANSPORT_LE)
        if (!ready.await(CONNECT_TIMEOUT_SECONDS, TimeUnit.SECONDS)) {
            gatt.close()
            throw IOException("nearby connection timed out")
        }
        failure?.let {
            gatt.close()
            throw it
        }
        return PairedBleLink(checkNotNull(channel), endpoint)
    }

    @SuppressLint("MissingPermission")
    private fun enableNotifications(gatt: BluetoothGatt, characteristic: BluetoothGattCharacteristic): Boolean {
        gatt.setCharacteristicNotification(characteristic, true)
        val descriptor = characteristic.getDescriptor(CLIENT_CONFIG_UUID) ?: return false
        return if (Build.VERSION.SDK_INT >= 33) {
            gatt.writeDescriptor(descriptor, BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE) ==
                android.bluetooth.BluetoothStatusCodes.SUCCESS
        } else {
            @Suppress("DEPRECATION")
            descriptor.value = BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE
            @Suppress("DEPRECATION")
            gatt.writeDescriptor(descriptor)
        }
    }

    private companion object {
        const val CONNECT_TIMEOUT_SECONDS = 35L
        const val ENDPOINT_RETRY_MILLIS = 500L
        const val MAX_ENDPOINT_READ_ATTEMPTS = 60
        val CLIENT_CONFIG_UUID: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")
    }
}

private class GattClientFrameChannel(
    private val gatt: BluetoothGatt,
    private val characteristic: BluetoothGattCharacteristic,
) : FrameChannel {
    private val pending = ArrayDeque<ByteArray>()
    private val decoder = BleFrameCodec.Decoder()
    private var receiver: (ByteArray) -> Unit = {}
    private var writing = false
    private var open = true

    @Synchronized
    override fun send(frame: ByteArray) {
        if (!open) throw IOException("disconnected")
        BleFrameCodec.chunk(frame).forEach(pending::addLast)
        if (!writing) writeNext()
    }

    @Synchronized
    override fun onReceive(receiver: (ByteArray) -> Unit) {
        this.receiver = receiver
    }

    @Synchronized
    fun receiveChunk(chunk: ByteArray) {
        try {
            decoder.receive(chunk)?.let(receiver)
        } catch (_: IllegalArgumentException) {
            close()
        }
    }

    @Synchronized
    fun writeCompleted(success: Boolean) {
        if (!success) {
            close()
            return
        }
        writing = false
        writeNext()
    }

    fun remoteDisconnected() = close()

    @SuppressLint("MissingPermission")
    @Synchronized
    override fun close() {
        if (!open) return
        open = false
        pending.clear()
        gatt.disconnect()
        gatt.close()
    }

    @SuppressLint("MissingPermission")
    private fun writeNext() {
        val chunk = pending.pollFirst() ?: return
        writing = true
        val result = if (Build.VERSION.SDK_INT >= 33) {
            gatt.writeCharacteristic(
                characteristic,
                chunk,
                BluetoothGattCharacteristic.WRITE_TYPE_DEFAULT,
            ) == android.bluetooth.BluetoothStatusCodes.SUCCESS
        } else {
            @Suppress("DEPRECATION")
            characteristic.value = chunk
            @Suppress("DEPRECATION")
            gatt.writeCharacteristic(characteristic)
        }
        if (!result) {
            writing = false
            close()
            throw IOException("nearby write failed")
        }
    }
}

object LocalEndpointAdvertisement {
    private const val MAX_HOST_BYTES = 45

    fun encode(endpoint: LocalEndpoint): ByteArray {
        val host = endpoint.host.toByteArray(StandardCharsets.US_ASCII)
        require(host.size in 1..MAX_HOST_BYTES) { "invalid endpoint host" }
        return ByteBuffer.allocate(1 + host.size + 2)
            .order(ByteOrder.BIG_ENDIAN)
            .put(host.size.toByte())
            .put(host)
            .putShort(endpoint.port.toShort())
            .array()
    }

    fun decode(bytes: ByteArray): LocalEndpoint? = runCatching {
        val buffer = ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN)
        val hostLength = buffer.get().toInt() and 0xff
        require(hostLength in 1..MAX_HOST_BYTES && buffer.remaining() == hostLength + 2)
        val host = ByteArray(hostLength).also(buffer::get).toString(StandardCharsets.US_ASCII)
        val port = buffer.short.toInt() and 0xffff
        LocalEndpoint(host, port)
    }.getOrNull()
}
