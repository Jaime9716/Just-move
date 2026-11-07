package com.juegodebaile.mando.core

// Codificación/decodificación de los mensajes UDP móvil <-> PC (contracts/phone_protocol.md).
// Lógica pura: sin android.*. Todo little-endian. Debe producir exactamente los mismos bytes que
// engine/jd_core/src/phone/protocol.rs (lo comprueban los tests con protocol_vectors.json).
//
// Convenio de tipos: los campos u32 del contrato (device_id, seq, pairing_code, source_id) se guardan
// en Int con los mismos 32 bits (p. ej. 0xDEADBEEF.toInt()); los u16/u8 en Int (0..65535 / 0..255).

import java.nio.ByteBuffer
import java.nio.ByteOrder

object Protocol {
    val MAGIC: ByteArray = byteArrayOf('J'.code.toByte(), 'D'.code.toByte(), 'M'.code.toByte(), '1'.code.toByte())
    const val VERSION: Int = 1
    const val HEADER_LEN: Int = 16
    const val SAMPLE_LEN: Int = 36
    const val DEFAULT_PORT: Int = 47800
    /** Máximo de muestras por paquete que acepta el PC (MAX_SAMPLES_PER_PACKET en protocol.rs). */
    const val MAX_SAMPLES_PER_PACKET: Int = 64
    /** Máximo que envía este móvil por paquete (contrato: ≤ 8). */
    const val MAX_SAMPLES_SENT: Int = 8

    const val T_HELLO: Int = 1
    const val T_WELCOME: Int = 2
    const val T_SAMPLES: Int = 3
    const val T_SYNC_REQ: Int = 4
    const val T_SYNC_RESP: Int = 5
    const val T_STATUS: Int = 6
    const val T_BYE: Int = 7
    const val T_DISCOVER: Int = 8

    const val FLAG_HAS_ACCEL: Int = 1
    const val FLAG_HAS_GYRO: Int = 2

    /** JD_Limb (contracts/motion_sample.h). */
    const val LIMB_LEFT_HAND: Int = 0
    const val LIMB_RIGHT_HAND: Int = 1
    const val LIMB_LEFT_FOOT: Int = 2
    const val LIMB_RIGHT_FOOT: Int = 3
    const val LIMB_NONE: Int = 255

    /** Nombre en español de la extremidad para la UI. */
    fun limbNameEs(limb: Int): String = when (limb) {
        LIMB_LEFT_HAND -> "mano izquierda"
        LIMB_RIGHT_HAND -> "mano derecha"
        LIMB_LEFT_FOOT -> "pie izquierdo"
        LIMB_RIGHT_FOOT -> "pie derecho"
        else -> "sin extremidad"
    }

    private fun alloc(bodyLen: Int): ByteBuffer =
        ByteBuffer.allocate(HEADER_LEN + bodyLen).order(ByteOrder.LITTLE_ENDIAN)

    private fun putHeader(b: ByteBuffer, type: Int, deviceId: Int, seq: Int) {
        b.put(MAGIC)
        b.put(type.toByte())
        b.put(VERSION.toByte())
        b.putShort(0)
        b.putInt(deviceId)
        b.putInt(seq)
    }

    /** Trunca a ≤ 255 bytes UTF-8 sin cortar un carácter (como protocol.rs). */
    fun truncateUtf8(name: String, maxBytes: Int = 255): ByteArray {
        val all = name.toByteArray(Charsets.UTF_8)
        if (all.size <= maxBytes) return all
        var n = maxBytes
        // Retrocede mientras el byte en n sea de continuación (10xxxxxx): n no es límite de carácter.
        while (n > 0 && (all[n].toInt() and 0xC0) == 0x80) n--
        return all.copyOf(n)
    }

    fun encode(p: Packet): ByteArray {
        val m = p.msg
        val b: ByteBuffer
        when (m) {
            is Message.Hello -> {
                val name = truncateUtf8(m.name)
                b = alloc(5 + name.size)
                putHeader(b, T_HELLO, p.deviceId, p.seq)
                b.putInt(m.pairingCode)
                b.put(name.size.toByte())
                b.put(name)
            }
            is Message.Welcome -> {
                b = alloc(6)
                putHeader(b, T_WELCOME, p.deviceId, p.seq)
                b.putInt(m.sourceId)
                b.put(m.limb.toByte())
                b.put((if (m.accepted) 1 else 0).toByte())
            }
            is Message.Samples -> {
                val count = minOf(m.samples.size, 0xFFFF)
                b = alloc(4 + count * SAMPLE_LEN)
                putHeader(b, T_SAMPLES, p.deviceId, p.seq)
                b.putShort(count.toShort())
                b.putShort(0)
                for (i in 0 until count) putSample(b, m.samples[i])
            }
            is Message.SyncReq -> {
                b = alloc(8)
                putHeader(b, T_SYNC_REQ, p.deviceId, p.seq)
                b.putLong(m.tPcSend)
            }
            is Message.SyncResp -> {
                b = alloc(24)
                putHeader(b, T_SYNC_RESP, p.deviceId, p.seq)
                b.putLong(m.tPcSend)
                b.putLong(m.tPhoneRecv)
                b.putLong(m.tPhoneSend)
            }
            is Message.Status -> {
                b = alloc(8)
                putHeader(b, T_STATUS, p.deviceId, p.seq)
                b.putFloat(m.battery)
                b.putShort(m.sensorRateHz.toShort())
                b.putShort(m.dropped.toShort())
            }
            Message.Bye -> {
                b = alloc(0)
                putHeader(b, T_BYE, p.deviceId, p.seq)
            }
            Message.Discover -> {
                b = alloc(0)
                putHeader(b, T_DISCOVER, p.deviceId, p.seq)
            }
        }
        return b.array()
    }

    private fun putSample(b: ByteBuffer, s: PhoneSample) {
        b.putLong(s.tNs)
        b.put(s.flags.toByte())
        b.put(0); b.put(0); b.put(0)
        b.putFloat(s.ax); b.putFloat(s.ay); b.putFloat(s.az)
        b.putFloat(s.gx); b.putFloat(s.gy); b.putFloat(s.gz)
    }

    /**
     * Decodifica `len` bytes de `data` desde `offset`. Nunca lanza: datos basura -> null.
     * Mismas reglas de validación que protocol.rs (longitud exacta por tipo).
     */
    fun decode(data: ByteArray, offset: Int = 0, len: Int = data.size - offset): Packet? {
        if (offset < 0 || len < 0 || offset + len > data.size) return null
        if (len < HEADER_LEN) return null
        for (i in 0 until 4) if (data[offset + i] != MAGIC[i]) return null
        val b = ByteBuffer.wrap(data, offset, len).slice().order(ByteOrder.LITTLE_ENDIAN)
        val type = b.get(4).toInt() and 0xFF
        val version = b.get(5).toInt() and 0xFF
        if (version != VERSION) return null
        val deviceId = b.getInt(8)
        val seq = b.getInt(12)
        val body = len - HEADER_LEN
        b.position(HEADER_LEN)
        val msg: Message = when (type) {
            T_HELLO -> {
                if (body < 5) return null
                val code = b.getInt()
                val n = b.get().toInt() and 0xFF
                if (body != 5 + n) return null
                val nameBytes = ByteArray(n)
                b.get(nameBytes)
                Message.Hello(code, String(nameBytes, Charsets.UTF_8))
            }
            T_WELCOME -> {
                if (body != 6) return null
                val sid = b.getInt()
                val limb = b.get().toInt() and 0xFF
                val acc = b.get().toInt() != 0
                Message.Welcome(sid, limb, acc)
            }
            T_SAMPLES -> {
                if (body < 4) return null
                val count = b.getShort().toInt() and 0xFFFF
                b.getShort()
                if (count > MAX_SAMPLES_PER_PACKET) return null
                if (body != 4 + count * SAMPLE_LEN) return null
                val list = ArrayList<PhoneSample>(count)
                for (i in 0 until count) {
                    val t = b.getLong()
                    val flags = b.get().toInt() and 0xFF
                    b.get(); b.get(); b.get()
                    list.add(
                        PhoneSample(
                            t, flags,
                            b.getFloat(), b.getFloat(), b.getFloat(),
                            b.getFloat(), b.getFloat(), b.getFloat()
                        )
                    )
                }
                Message.Samples(list)
            }
            T_SYNC_REQ -> {
                if (body != 8) return null
                Message.SyncReq(b.getLong())
            }
            T_SYNC_RESP -> {
                if (body != 24) return null
                Message.SyncResp(b.getLong(), b.getLong(), b.getLong())
            }
            T_STATUS -> {
                if (body != 8) return null
                val bat = b.getFloat()
                val rate = b.getShort().toInt() and 0xFFFF
                val dropped = b.getShort().toInt() and 0xFFFF
                Message.Status(bat, rate, dropped)
            }
            T_BYE -> {
                if (body != 0) return null
                Message.Bye
            }
            T_DISCOVER -> {
                if (body != 0) return null
                Message.Discover
            }
            else -> return null
        }
        return Packet(deviceId, seq, msg)
    }
}

/** Muestra tal como se envía (t_ns en la base de SystemClock.elapsedRealtimeNanos()). */
data class PhoneSample(
    val tNs: Long,
    val flags: Int,
    val ax: Float, val ay: Float, val az: Float,
    val gx: Float, val gy: Float, val gz: Float,
)

sealed class Message {
    data class Hello(val pairingCode: Int, val name: String) : Message()
    data class Welcome(val sourceId: Int, val limb: Int, val accepted: Boolean) : Message()
    data class Samples(val samples: List<PhoneSample>) : Message()
    data class SyncReq(val tPcSend: Long) : Message()
    data class SyncResp(val tPcSend: Long, val tPhoneRecv: Long, val tPhoneSend: Long) : Message()
    data class Status(val battery: Float, val sensorRateHz: Int, val dropped: Int) : Message()
    object Bye : Message() {
        override fun toString(): String = "Bye"
    }
    object Discover : Message() {
        override fun toString(): String = "Discover"
    }
}

data class Packet(val deviceId: Int, val seq: Int, val msg: Message)
