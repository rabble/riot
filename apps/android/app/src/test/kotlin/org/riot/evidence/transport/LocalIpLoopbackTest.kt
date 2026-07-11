package org.riot.evidence.transport

import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertNotNull
import org.junit.Test

class LocalIpLoopbackTest {
    @Test
    fun listenerAdvertisesEndpointAndAcceptsSingleBoundedLocalConnection() {
        LocalIpListener("127.0.0.1").use { listener ->
            val received = arrayOfNulls<ByteArray>(1)
            val done = CountDownLatch(1)
            val client = SocketLocalIpConnector().connect(listener.endpoint)
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
}
