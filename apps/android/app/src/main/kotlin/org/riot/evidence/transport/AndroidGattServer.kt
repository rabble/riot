package org.riot.evidence.transport

import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothGatt
import android.bluetooth.BluetoothGattCharacteristic
import android.bluetooth.BluetoothGattDescriptor
import android.bluetooth.BluetoothGattServer
import android.bluetooth.BluetoothGattServerCallback
import android.bluetooth.BluetoothGattService
import android.bluetooth.BluetoothManager
import android.content.Context
import android.os.Build
import java.io.IOException
import java.util.UUID

// AndroidNearbyController constructs this boundary only after all version-specific runtime grants succeed.
@SuppressLint("MissingPermission")
class AndroidGattServer(
    context: Context,
    private val endpoint: LocalEndpoint?,
    private val onPairingRequested: (DiscoveredPhone, PairedBleLink) -> Unit,
) : AutoCloseable {
    private val manager = context.getSystemService(BluetoothManager::class.java)
        ?: throw IllegalStateException("Nearby Bluetooth is unavailable")
    private val channels = mutableMapOf<String, GattServerFrameChannel>()
    private val discoveredPhones = mutableMapOf<String, DiscoveredPhone>()
    private val requested = mutableSetOf<String>()
    private val callback = object : BluetoothGattServerCallback() {
        override fun onConnectionStateChange(device: BluetoothDevice, status: Int, newState: Int) {
            if (status == BluetoothGatt.GATT_SUCCESS && newState == android.bluetooth.BluetoothProfile.STATE_CONNECTED) {
                channels[device.address] = GattServerFrameChannel(server, device)
                requestIfIdentified(device.address)
            } else {
                channels.remove(device.address)?.remoteDisconnected()
                requested.remove(device.address)
            }
        }

        override fun onCharacteristicReadRequest(
            device: BluetoothDevice,
            requestId: Int,
            offset: Int,
            characteristic: BluetoothGattCharacteristic,
        ) {
            val channel = channels[device.address]
            if (characteristic.uuid != AndroidBleDiscovery.ENDPOINT_UUID || channel?.confirmed != true) {
                server.sendResponse(device, requestId, BluetoothGatt.GATT_READ_NOT_PERMITTED, offset, null)
                return
            }
            val value = endpoint?.let(LocalEndpointAdvertisement::encode) ?: byteArrayOf()
            server.sendResponse(device, requestId, BluetoothGatt.GATT_SUCCESS, offset, value)
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
            val channel = channels[device.address]
            val accepted = characteristic.uuid == AndroidBleDiscovery.DATA_UUID &&
                !preparedWrite && offset == 0 && channel?.confirmed == true && channel.receiveChunk(value)
            if (responseNeeded) {
                server.sendResponse(
                    device,
                    requestId,
                    if (accepted) BluetoothGatt.GATT_SUCCESS else BluetoothGatt.GATT_WRITE_NOT_PERMITTED,
                    offset,
                    null,
                )
            }
        }

        override fun onDescriptorWriteRequest(
            device: BluetoothDevice,
            requestId: Int,
            descriptor: BluetoothGattDescriptor,
            preparedWrite: Boolean,
            responseNeeded: Boolean,
            offset: Int,
            value: ByteArray,
        ) {
            val accepted = descriptor.uuid == CLIENT_CONFIG_UUID &&
                value.contentEquals(BluetoothGattDescriptor.ENABLE_NOTIFICATION_VALUE)
            channels[device.address]?.subscribed = accepted
            if (responseNeeded) {
                server.sendResponse(
                    device,
                    requestId,
                    if (accepted) BluetoothGatt.GATT_SUCCESS else BluetoothGatt.GATT_WRITE_NOT_PERMITTED,
                    offset,
                    null,
                )
            }
        }

        override fun onNotificationSent(device: BluetoothDevice, status: Int) {
            channels[device.address]?.notificationSent(status == BluetoothGatt.GATT_SUCCESS)
        }
    }
    private val server: BluetoothGattServer = manager.openGattServer(context, callback)
        ?: throw IllegalStateException("Nearby service could not start")

    init {
        val data = BluetoothGattCharacteristic(
            AndroidBleDiscovery.DATA_UUID,
            BluetoothGattCharacteristic.PROPERTY_WRITE or BluetoothGattCharacteristic.PROPERTY_NOTIFY,
            BluetoothGattCharacteristic.PERMISSION_WRITE,
        ).apply {
            addDescriptor(
                BluetoothGattDescriptor(
                    CLIENT_CONFIG_UUID,
                    BluetoothGattDescriptor.PERMISSION_READ or BluetoothGattDescriptor.PERMISSION_WRITE,
                ),
            )
        }
        val endpointCharacteristic = BluetoothGattCharacteristic(
            AndroidBleDiscovery.ENDPOINT_UUID,
            BluetoothGattCharacteristic.PROPERTY_READ,
            BluetoothGattCharacteristic.PERMISSION_READ,
        )
        server.addService(
            BluetoothGattService(
                AndroidBleDiscovery.SERVICE_UUID,
                BluetoothGattService.SERVICE_TYPE_PRIMARY,
            ).apply {
                addCharacteristic(data)
                addCharacteristic(endpointCharacteristic)
            },
        )
    }

    fun phoneDiscovered(phone: DiscoveredPhone) {
        discoveredPhones[phone.handle.value] = phone
        requestIfIdentified(phone.handle.value)
    }

    fun confirm(handle: PeerHandle): PairedBleLink {
        val channel = checkNotNull(channels[handle.value]) { "Nearby phone disconnected" }
        channel.confirmed = true
        return PairedBleLink(channel, null)
    }

    fun reject(handle: PeerHandle) {
        channels.remove(handle.value)?.close()
        requested.remove(handle.value)
    }

    @SuppressLint("MissingPermission")
    override fun close() {
        channels.values.toList().forEach(GattServerFrameChannel::close)
        channels.clear()
        server.close()
    }

    private fun requestIfIdentified(address: String) {
        val phone = discoveredPhones[address] ?: return
        val channel = channels[address] ?: return
        if (!requested.add(address)) return
        onPairingRequested(
            phone,
            PairedBleLink(channel, null),
        )
    }

    private companion object {
        val CLIENT_CONFIG_UUID: UUID = UUID.fromString("00002902-0000-1000-8000-00805f9b34fb")
    }
}

private class GattServerFrameChannel(
    private val server: BluetoothGattServer,
    private val device: BluetoothDevice,
) : FrameChannel {
    private val pending = BleOutboundQueue()
    private val decoder = BleFrameCodec.Decoder()
    private val deferredReceiver = DeferredFrameReceiver()
    private var notifying = false
    private var open = true
    var confirmed = false
    var subscribed = false

    @Synchronized
    override fun send(frame: ByteArray) {
        if (!open || !confirmed) throw IOException("nearby connection not confirmed")
        if (!subscribed) throw IOException("nearby notifications unavailable")
        try {
            pending.add(frame)
        } catch (error: IOException) {
            close()
            throw error
        }
        if (!notifying) notifyNext()
    }

    @Synchronized
    override fun onReceive(receiver: (ByteArray) -> Unit) {
        deferredReceiver.register(receiver)
    }

    @Synchronized
    fun receiveChunk(chunk: ByteArray): Boolean = try {
        decoder.receive(chunk)?.let(deferredReceiver::deliver)
        true
    } catch (_: Exception) {
        close()
        false
    }

    @Synchronized
    fun notificationSent(success: Boolean) {
        if (!success) {
            close()
            return
        }
        notifying = false
        notifyNext()
    }

    fun remoteDisconnected() {
        open = false
        pending.close()
        deferredReceiver.close()
    }

    @SuppressLint("MissingPermission")
    @Synchronized
    override fun close() {
        if (!open) return
        open = false
        pending.close()
        deferredReceiver.close()
        server.cancelConnection(device)
    }

    @SuppressLint("MissingPermission")
    private fun notifyNext() {
        val chunk = pending.pollChunk() ?: return
        val service = server.getService(AndroidBleDiscovery.SERVICE_UUID)
        val characteristic = service?.getCharacteristic(AndroidBleDiscovery.DATA_UUID)
            ?: throw IOException("nearby data characteristic unavailable")
        notifying = true
        val success = if (Build.VERSION.SDK_INT >= 33) {
            server.notifyCharacteristicChanged(device, characteristic, false, chunk) ==
                android.bluetooth.BluetoothStatusCodes.SUCCESS
        } else {
            @Suppress("DEPRECATION")
            characteristic.value = chunk
            @Suppress("DEPRECATION")
            server.notifyCharacteristicChanged(device, characteristic, false)
        }
        if (!success) {
            notifying = false
            close()
            throw IOException("nearby notification failed")
        }
    }
}
