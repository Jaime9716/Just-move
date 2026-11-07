package com.juegodebaile.mando.core

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Compara el codificador Kotlin con bytes generados por el codificador REAL del PC
 * (jd_core::phone::protocol::encode). Si este test falla, la app y el PC no se entienden.
 */
class ProtocolVectorsTest {

    @Suppress("UNCHECKED_CAST")
    private fun loadVectors(): List<Map<String, Any?>> {
        val text = javaClass.classLoader?.getResourceAsStream("protocol_vectors.json")
            ?.use { it.readBytes().toString(Charsets.UTF_8) }
            ?: File("src/test/resources/protocol_vectors.json").readText(Charsets.UTF_8)
        val root = MiniJson.parse(text) as Map<String, Any?>
        return root["vectors"] as List<Map<String, Any?>>
    }

    private fun hexToBytes(h: String): ByteArray {
        require(h.length % 2 == 0)
        return ByteArray(h.length / 2) { k -> h.substring(2 * k, 2 * k + 2).toInt(16).toByte() }
    }

    private fun n(v: Map<String, Any?>, k: String): JsonNum = v[k] as JsonNum

    @Suppress("UNCHECKED_CAST")
    private fun floats3(x: Any?): FloatArray = (x as List<Any?>).map { (it as JsonNum).toFloat() }.toFloatArray()

    @Suppress("UNCHECKED_CAST")
    private fun messageOf(v: Map<String, Any?>): Message = when (v["type"] as String) {
        "HELLO" -> Message.Hello(n(v, "pairing_code").toU32Bits(), v["name"] as String)
        "WELCOME" -> Message.Welcome(n(v, "source_id").toU32Bits(), n(v, "limb").toInt(), v["accepted"] as Boolean)
        "SAMPLES" -> Message.Samples((v["samples"] as List<Map<String, Any?>>).map { s ->
            val a = floats3(s["accel"])
            val g = floats3(s["gyro"])
            PhoneSample(n(s, "t_ns").toLong(), n(s, "flags").toInt(), a[0], a[1], a[2], g[0], g[1], g[2])
        })
        "SYNC_REQ" -> Message.SyncReq(n(v, "t_pc_send").toLong())
        "SYNC_RESP" -> Message.SyncResp(n(v, "t_pc_send").toLong(), n(v, "t_phone_recv").toLong(), n(v, "t_phone_send").toLong())
        "STATUS" -> Message.Status(n(v, "battery").toFloat(), n(v, "sensor_rate_hz").toInt(), n(v, "dropped").toInt())
        "BYE" -> Message.Bye
        "DISCOVER" -> Message.Discover
        else -> throw AssertionError("tipo desconocido ${v["type"]}")
    }

    @Test
    fun hayVectoresDeTodosLosTipos() {
        val types = loadVectors().map { it["type"] as String }.toSet()
        assertEquals(setOf("HELLO", "WELCOME", "SAMPLES", "SYNC_REQ", "SYNC_RESP", "STATUS", "BYE", "DISCOVER"), types)
    }

    @Test
    fun codificaExactamenteLosBytesDelPc() {
        for (v in loadVectors()) {
            val id = v["id"] as String
            val p = Packet(n(v, "device_id").toU32Bits(), n(v, "seq").toU32Bits(), messageOf(v))
            val want = hexToBytes(v["hex"] as String)
            assertArrayEquals("vector $id", want, Protocol.encode(p))
        }
    }

    @Test
    fun decodificaLosBytesDelPc() {
        for (v in loadVectors()) {
            val id = v["id"] as String
            val bytes = hexToBytes(v["hex"] as String)
            val got = Protocol.decode(bytes)
            assertNotNull("vector $id", got)
            got!!
            assertEquals("device_id $id", n(v, "device_id").toU32Bits(), got.deviceId)
            assertEquals("seq $id", n(v, "seq").toU32Bits(), got.seq)
            // Comparación por bits (incluye -0.0): re-codificar debe dar los mismos bytes.
            assertArrayEquals("re-codificado $id", bytes, Protocol.encode(got))
            assertEquals("mensaje $id", messageOf(v).javaClass, got.msg.javaClass)
            if (got.msg is Message.Hello) assertEquals(v["name"], (got.msg as Message.Hello).name)
        }
    }

    @Test
    fun longitudesDelContrato() {
        for (v in loadVectors()) {
            val len = (v["hex"] as String).length / 2
            assertTrue(len >= Protocol.HEADER_LEN)
            if (v["type"] == "SAMPLES") {
                val count = (v["samples"] as List<*>).size
                assertEquals(Protocol.HEADER_LEN + 4 + count * Protocol.SAMPLE_LEN, len)
            }
        }
    }
}
