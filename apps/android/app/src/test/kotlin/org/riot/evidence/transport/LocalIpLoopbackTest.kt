package org.riot.evidence.transport

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.io.IOException
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test

class LocalIpLoopbackTest {
    @Test
    fun listenerAdvertisesEndpointAndAcceptsSingleBoundedLocalConnection() {
        LocalIpListener("127.0.0.1").use { listener ->
            val received = arrayOfNulls<ByteArray>(1)
            val done = CountDownLatch(1)
            val client = SocketLocalIpConnector.loopbackForTest().connect(listener.endpoint)
            val server = listener.awaitAccepted(2_000)
            assertNotNull(server)
            server!!.onReceive {
                received[0] = it
                done.countDown()
            }

            client.send(byteArrayOf(1, 3, 5, 7))

            assert(done.await(2, TimeUnit.SECONDS))
            assertArrayEquals(byteArrayOf(1, 3, 5, 7), received[0])
            client.close()
            server.close()
        }
    }

    @Test
    fun frameSentBeforeSocketReceiverRegistrationIsDeliveredOnce() {
        LocalIpListener("127.0.0.1").use { listener ->
            val client = SocketLocalIpConnector.loopbackForTest().connect(listener.endpoint)
            val server = listener.awaitAccepted(2_000)!!
            client.send(byteArrayOf(9, 7, 5))
            val received = mutableListOf<ByteArray>()
            val done = CountDownLatch(1)

            server.onReceive { received += it; done.countDown() }

            assert(done.await(2, TimeUnit.SECONDS))
            assertEquals(1, received.size)
            assertArrayEquals(byteArrayOf(9, 7, 5), received.single())
            client.close()
            server.close()
        }
    }

    @Test
    fun closedListenerRejectsAnyLateSecondTransport() {
        val listener = LocalIpListener("127.0.0.1")
        val endpoint = listener.endpoint
        listener.close()

        org.junit.Assert.assertThrows(IOException::class.java) {
            SocketLocalIpConnector.loopbackForTest(100).connect(endpoint)
        }
    }
}
