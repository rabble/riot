package org.riot.evidence.transport

import java.io.DataInputStream
import java.io.DataOutputStream
import java.io.IOException
import java.net.Inet4Address
import java.net.Inet6Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.NetworkInterface
import java.net.ServerSocket
import java.net.Socket
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

class SocketLocalIpConnector(private val timeoutMillis: Int = 1_500) : LocalIpConnector {
    override fun connect(endpoint: LocalEndpoint): FrameChannel {
        val address = numericLocalAddress(endpoint.host)
        val socket = Socket()
        try {
            socket.connect(InetSocketAddress(address, endpoint.port), timeoutMillis)
            return SocketFrameChannel(socket)
        } catch (error: IOException) {
            socket.close()
            throw error
        }
    }

    private fun numericLocalAddress(host: String): InetAddress {
        val address = when {
            IPV4.matches(host) -> {
                val octets = host.split('.').map { part ->
                    part.toIntOrNull()?.takeIf { it in 0..255 }
                        ?: throw IOException("invalid numeric local address")
                }
                InetAddress.getByAddress(octets.map(Int::toByte).toByteArray())
            }
            ':' in host && IPV6.matches(host) -> InetAddress.getByName(host)
            else -> throw IOException("local endpoint must be a numeric IP address")
        }
        val allowed = address.isLoopbackAddress || address.isLinkLocalAddress || when (address) {
            is Inet4Address -> address.isSiteLocalAddress
            is Inet6Address -> (address.address.first().toInt() and 0xfe) == 0xfc
            else -> false
        }
        if (!allowed) throw IOException("endpoint is outside the local network")
        return address
    }

    private companion object {
        val IPV4 = Regex("[0-9]{1,3}(\\.[0-9]{1,3}){3}")
        val IPV6 = Regex("[0-9a-fA-F:]+")
    }
}

class LocalIpListener(host: String) : AutoCloseable {
    private val accepted = ArrayBlockingQueue<FrameChannel>(1)
    private val server = ServerSocket(0, 1, InetAddress.getByName(host))
    val endpoint = LocalEndpoint(host, server.localPort)

    init {
        thread(name = "riot-local-accept", isDaemon = true) {
            try {
                accepted.offer(SocketFrameChannel(server.accept()))
            } catch (_: IOException) {
                // Closing the listener is the normal cancellation path.
            } finally {
                runCatching(server::close)
            }
        }
    }

    fun awaitAccepted(timeoutMillis: Long): FrameChannel? =
        accepted.poll(timeoutMillis, TimeUnit.MILLISECONDS)

    override fun close() {
        runCatching(server::close)
        accepted.poll()?.close()
    }

    companion object {
        fun forDevice(): LocalIpListener = LocalIpListener(findDeviceLocalAddress())

        private fun findDeviceLocalAddress(): String {
            val addresses = NetworkInterface.getNetworkInterfaces()?.toList().orEmpty()
                .flatMap { it.inetAddresses.toList() }
            return addresses.firstOrNull {
                !it.isLoopbackAddress && (it.isSiteLocalAddress || it.isLinkLocalAddress)
            }?.hostAddress?.substringBefore('%')
                ?: throw IOException("No direct local network address")
        }
    }
}

internal class SocketFrameChannel(private val socket: Socket) : FrameChannel {
    private val input = DataInputStream(socket.getInputStream())
    private val output = DataOutputStream(socket.getOutputStream())
    @Volatile private var receiver: (ByteArray) -> Unit = {}
    @Volatile private var open = true

    init {
        thread(name = "riot-local-reader", isDaemon = true) {
            try {
                while (open) {
                    val length = input.readInt()
                    if (length !in 0..MAX_SYNC_FRAME_BYTES) throw IOException("invalid local frame length")
                    val frame = ByteArray(length)
                    input.readFully(frame)
                    receiver(frame)
                }
            } catch (_: IOException) {
                close()
            }
        }
    }

    @Synchronized
    override fun send(frame: ByteArray) {
        if (!open) throw IOException("disconnected")
        if (frame.size > MAX_SYNC_FRAME_BYTES) throw IOException("frame is too large")
        output.writeInt(frame.size)
        output.write(frame)
        output.flush()
    }

    override fun onReceive(receiver: (ByteArray) -> Unit) {
        this.receiver = receiver
    }

    @Synchronized
    override fun close() {
        if (!open) return
        open = false
        runCatching(socket::close)
    }
}
