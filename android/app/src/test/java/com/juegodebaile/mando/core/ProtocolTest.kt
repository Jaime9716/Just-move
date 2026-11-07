package com.juegodebaile.mando.core

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ProtocolTest {
    private val all: List<Message> = listOf(
        Message.Hello(1234, "Pixel 8 Pro ñ"),
        Message.Hello(0, ""),
        Message.Welcome(3, 1, true),
        Message.Welcome(0, 255, false),
        Message.Samples(emptyList()),
        Message.Samples((0 until 8).map { k ->
            PhoneSample(-5L + k * 5_000_000L, 3, k.toFloat(), 9.81f, -0.5f, 0.25f, -1f, 1e-3f)
        }),
        Message.SyncReq(123_456_789_012L),
        Message.SyncResp(1L, -2L, Long.MAX_VALUE),
        Message.Status(0.75f, 200, 3),
        Message.Bye,
        Message.Discover,
    )

    @Test
    fun idaYVueltaTodos() {
        for ((i, m) in all.withIndex()) {
            val p = Packet(0xDEADBEEF.toInt(), i, m)
            val b = Protocol.encode(p)
            assertEquals('J'.code.toByte(), b[0])
            assertEquals(1.toByte(), b[5])
            assertEquals(p, Protocol.decode(b))
        }
    }

    @Test
    fun cabeceraExacta() {
        val b = Protocol.encode(Packet(0x01020304, 0x0A0B0C0D, Message.SyncReq(0x1122334455667788L)))
        val want = byteArrayOf(
            'J'.code.toByte(), 'D'.code.toByte(), 'M'.code.toByte(), '1'.code.toByte(), 4, 1, 0, 0,
            4, 3, 2, 1, 0x0D, 0x0C, 0x0B, 0x0A,
            0x88.toByte(), 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11,
        )
        assertArrayEquals(want, b)
        val one = PhoneSample(0, 0, 0f, 0f, 0f, 0f, 0f, 0f)
        assertEquals(16 + 4 + 36, Protocol.encode(Packet(1, 2, Message.Samples(listOf(one)))).size)
        assertEquals(22, Protocol.encode(Packet(1, 2, Message.Welcome(1, 1, true))).size)
        assertEquals(24, Protocol.encode(Packet(1, 2, Message.Status(1f, 1, 1))).size)
        assertEquals(40, Protocol.encode(Packet(1, 2, Message.SyncResp(0, 0, 0))).size)
        assertEquals(16, Protocol.encode(Packet(1, 2, Message.Bye)).size)
    }

    @Test
    fun rechazaBasura() {
        val good = Protocol.encode(Packet(1, 1, Message.Status(0.5f, 200, 0)))
        assertNotNull(Protocol.decode(good))
        assertNull(Protocol.decode(good.copyOf(10)))
        assertNull(Protocol.decode(good.copyOf().also { it[0] = 'X'.code.toByte() }))
        assertNull(Protocol.decode(good.copyOf().also { it[5] = 2 }))
        assertNull(Protocol.decode(good.copyOf().also { it[4] = 99 }))
        assertNull(Protocol.decode(good.copyOf(good.size - 1)))
        assertNull(Protocol.decode(good.copyOf(good.size + 1)))
        val h = Protocol.encode(Packet(1, 1, Message.Hello(1, "abc")))
        h[20] = 200.toByte()
        assertNull(Protocol.decode(h))
        val s = Protocol.encode(Packet(1, 1, Message.Samples(List(2) { PhoneSample(0, 0, 0f, 0f, 0f, 0f, 0f, 0f) })))
        s[16] = 3
        assertNull(Protocol.decode(s))
        s[16] = 0xFF.toByte(); s[17] = 0xFF.toByte()
        assertNull(Protocol.decode(s))
        assertNull(Protocol.decode(ByteArray(0)))
        assertNull(Protocol.decode(good, 4, 100))
    }

    @Test
    fun decodificaConDesplazamiento() {
        val b = Protocol.encode(Packet(7, 9, Message.SyncReq(42L)))
        val buf = ByteArray(b.size + 10)
        System.arraycopy(b, 0, buf, 5, b.size)
        assertEquals(Packet(7, 9, Message.SyncReq(42L)), Protocol.decode(buf, 5, b.size))
    }

    @Test
    fun nuncaLanzaExcepcionConDatosAleatorios() {
        var x = 0x9E3779B97F4A7C15uL.toLong()
        fun rnd(): Long { x = x xor (x shl 13); x = x xor (x ushr 7); x = x xor (x shl 17); return x }
        val seeds = all.map { Protocol.encode(Packet(5, 9, it)) }
        for (i in 0 until 50_000) {
            val b: ByteArray = if (i % 3 == 0) {
                ByteArray((rnd() and 0x7F).toInt() % 80) { rnd().toByte() }
            } else {
                seeds[((rnd() ushr 1) % seeds.size).toInt()].copyOf()
            }
            var bb = b
            if (bb.isNotEmpty()) {
                repeat(((rnd() ushr 1) % 4).toInt()) { bb[((rnd() ushr 1) % bb.size).toInt()] = rnd().toByte() }
                if ((rnd() and 3L) == 0L) bb = bb.copyOf(((rnd() ushr 1) % (bb.size + 1)).toInt())
            }
            Protocol.decode(bb) // no debe lanzar
        }
    }

    @Test
    fun truncaNombreUtf8SinPartirCaracteres() {
        val name = "ñ".repeat(200) // 400 bytes
        val t = Protocol.truncateUtf8(name)
        assertEquals(254, t.size) // 127 eñes completas, no 255 bytes partidos
        assertEquals("ñ".repeat(127), String(t, Charsets.UTF_8))
        val p = Protocol.encode(Packet(1, 1, Message.Hello(1, name)))
        assertEquals(16 + 5 + 254, p.size)
        assertEquals(254, p[20].toInt() and 0xFF)
        assertEquals(255, Protocol.truncateUtf8("a".repeat(300)).size)
        assertTrue(Protocol.truncateUtf8("").isEmpty())
    }

    @Test
    fun u32ConBitAlto() {
        val p = Packet(0xFFFFFFFF.toInt(), 0x80000000.toInt(), Message.Hello(9999, "x"))
        val d = Protocol.decode(Protocol.encode(p))!!
        assertEquals(0xFFFFFFFFL, d.deviceId.toLong() and 0xFFFFFFFFL)
        assertEquals(0x80000000L, d.seq.toLong() and 0xFFFFFFFFL)
    }
}
