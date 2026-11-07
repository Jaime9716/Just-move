package com.juegodebaile.mando.core

// Cliente del protocolo móvil -> PC (contracts/phone_protocol.md), lógica pura y determinista:
// no abre sockets ni lee relojes por su cuenta salvo `clock` (para t_phone_send). La capa Android
// (NetThread) le pasa los datagramas recibidos, las muestras y un "tick" periódico, y envía lo que
// el cliente le pide por `sender`. Todo se llama desde UN solo hilo (el de red).
//
// Máquina de estados:
//   DISCOVERING --connect()--> PAIRING --WELCOME ok--> CONNECTED
//        ^                       |  ^                     |
//        |                       |  +----WELCOME ok-------+-- 3 s sin oír al PC / BYE del PC --> LOST
//        +----disconnect()-------+                                   (LOST re-envía HELLO cada 500 ms)
//   PAIRING/LOST --WELCOME rechazado--> REJECTED (no envía nada hasta un nuevo connect())

enum class ConnState { DISCOVERING, PAIRING, CONNECTED, REJECTED, LOST }

/** Reloj monotónico en la base de SystemClock.elapsedRealtimeNanos(). */
fun interface MonoClock {
    fun nowNs(): Long
}

/** Envía un datagrama. `dest` null = broadcast (puerto `broadcastPort`). Devuelve false si falló. */
fun interface Sender {
    fun send(bytes: ByteArray, dest: Endpoint?, broadcastPort: Int): Boolean
}

/** Foto inmutable del estado para la UI. */
data class ClientSnapshot(
    val state: ConnState,
    val target: Endpoint?,
    val discovered: Endpoint?,
    val sourceId: Int,
    val limb: Int,
    /** true si en PAIRING llevamos > 3 s sin oír nada del PC. */
    val noResponse: Boolean,
    val sampleRateHz: Float,
    /** rtt HELLO -> WELCOME (ms) del último emparejado; < 0 si no se conoce. */
    val pairRttMs: Float,
    val packetsSent: Long,
    val samplesSent: Long,
    val syncAnswered: Long,
    val dropped: Long,
    /** ms desde el último paquete del PC; < 0 si nunca. */
    val sinceHeardMs: Long,
)

class PhoneClient(
    val deviceId: Int,
    private val deviceName: String,
    private val clock: MonoClock,
    private val sender: Sender,
    private val batteryProvider: () -> Float = { -1f },
) {
    companion object {
        const val HELLO_PERIOD_NS = 500_000_000L
        const val DISCOVER_PERIOD_NS = 1_000_000_000L
        const val TIMEOUT_NS = 3_000_000_000L
        const val BATCH_PERIOD_NS = 10_000_000L
        const val STATUS_PERIOD_NS = 2_000_000_000L
        /** Un WELCOME rechazado que llega tan poco después de un DISCOVER puede ser la respuesta a este. */
        const val DISCOVER_ECHO_GUARD_NS = 300_000_000L
        const val MAX_BATCH = Protocol.MAX_SAMPLES_SENT
    }

    var state: ConnState = ConnState.DISCOVERING
        private set
    var target: Endpoint? = null
        private set
    private var pairingCode: Int = 0
    var discovered: Endpoint? = null
        private set
    var sourceId: Int = 0
        private set
    var limb: Int = Protocol.LIMB_NONE
        private set

    private var seq: Int = 0
    private var lastHelloNs: Long = Long.MIN_VALUE
    private var lastDiscoverNs: Long = Long.MIN_VALUE
    private var lastHeardNs: Long = Long.MIN_VALUE
    private var pairingStartNs: Long = 0L
    private var lastFlushNs: Long = Long.MIN_VALUE
    private var nextStatusNs: Long = Long.MIN_VALUE
    private val batch = ArrayList<PhoneSample>(MAX_BATCH)
    private val rate = RateMeter()
    private var pairRttNs: Long = -1L
    private var lastNowNs: Long = 0L

    var packetsSent: Long = 0
        private set
    var samplesSent: Long = 0
        private set
    var syncAnswered: Long = 0
        private set
    /** Muestras perdidas en el móvil (cola llena, envío fallido). Se informa saturado a u16 en STATUS. */
    var dropped: Long = 0
        private set

    /** Último seq usado (0 = aún no se ha enviado nada). */
    val lastSeq: Int get() = seq

    private fun nextSeq(): Int {
        seq += 1 // desborda de 0xFFFFFFFF a 0 igual que u32::wrapping_add
        return seq
    }

    private fun broadcastPort(): Int = target?.port ?: Protocol.DEFAULT_PORT

    private fun send(msg: Message, dest: Endpoint?): Boolean {
        val bytes = Protocol.encode(Packet(deviceId, nextSeq(), msg))
        val ok = sender.send(bytes, dest, broadcastPort())
        if (ok) packetsSent++
        return ok
    }

    // ------------------------------------------------------------------ órdenes de la UI

    /** Empieza a emparejar con `ep` usando `code`: HELLO inmediato y luego cada 500 ms. */
    fun connect(ep: Endpoint, code: Int, nowNs: Long) {
        lastNowNs = nowNs
        if (state == ConnState.CONNECTED || state == ConnState.LOST || state == ConnState.PAIRING) {
            if (target != null && target != ep) send(Message.Bye, target) // cambiamos de PC
        }
        target = ep
        pairingCode = code
        sourceId = 0
        limb = Protocol.LIMB_NONE
        pairRttNs = -1L
        batch.clear()
        enter(ConnState.PAIRING, nowNs)
        sendHello(nowNs)
    }

    /** Deja de enviar, se despide del PC (BYE) y vuelve a buscar PCs. */
    fun disconnect(nowNs: Long) {
        lastNowNs = nowNs
        val t = target
        if (t != null && (state == ConnState.CONNECTED || state == ConnState.PAIRING || state == ConnState.LOST)) {
            send(Message.Bye, t)
        }
        batch.clear()
        sourceId = 0
        limb = Protocol.LIMB_NONE
        enter(ConnState.DISCOVERING, nowNs)
        lastDiscoverNs = Long.MIN_VALUE // DISCOVER en el próximo tick
    }

    /** La capa Android informa de muestras que no llegaron al cliente (cola entre hilos llena). */
    fun noteDropped(n: Long) {
        if (n > 0) dropped += n
    }

    private fun enter(s: ConnState, nowNs: Long) {
        state = s
        when (s) {
            ConnState.PAIRING -> pairingStartNs = nowNs
            ConnState.CONNECTED -> {
                batch.clear()
                lastFlushNs = Long.MIN_VALUE
                nextStatusNs = nowNs // primer STATUS enseguida
            }
            ConnState.LOST -> batch.clear()
            ConnState.DISCOVERING, ConnState.REJECTED -> batch.clear()
        }
    }

    private fun sendHello(nowNs: Long) {
        val t = target ?: return
        send(Message.Hello(pairingCode, deviceName), t)
        lastHelloNs = nowNs
    }

    // ------------------------------------------------------------------ red

    /** Procesa un datagrama recibido en `tRecvNs` (elapsedRealtimeNanos justo tras recibirlo). */
    fun onDatagram(data: ByteArray, offset: Int, len: Int, fromHost: String, fromPort: Int, tRecvNs: Long) {
        lastNowNs = tRecvNs
        val p = Protocol.decode(data, offset, len) ?: return
        // El PC siempre contesta con NUESTRO device_id (server.rs); lo demás no es para nosotros.
        if (p.deviceId != deviceId) return
        when (val m = p.msg) {
            is Message.Welcome -> onWelcome(m, fromHost, fromPort, tRecvNs)
            is Message.SyncReq -> {
                if (state == ConnState.PAIRING || state == ConnState.CONNECTED || state == ConnState.LOST) {
                    lastHeardNs = tRecvNs
                    val t = target ?: return
                    // t_phone_send se toma lo más tarde posible, justo antes de codificar y enviar.
                    val tSend = clock.nowNs()
                    if (send(Message.SyncResp(m.tPcSend, tRecvNs, tSend), t)) syncAnswered++
                }
            }
            Message.Bye -> {
                if (state == ConnState.CONNECTED || state == ConnState.PAIRING) {
                    lastHeardNs = tRecvNs
                    enter(ConnState.LOST, tRecvNs)
                    lastHelloNs = tRecvNs // siguiente HELLO dentro de 500 ms
                }
            }
            else -> {} // HELLO/SAMPLES/STATUS/SYNC_RESP/DISCOVER: mensajes de móvil, se ignoran
        }
    }

    private fun onWelcome(m: Message.Welcome, fromHost: String, fromPort: Int, tRecvNs: Long) {
        if (m.accepted) {
            when (state) {
                ConnState.PAIRING, ConnState.LOST -> {
                    lastHeardNs = tRecvNs
                    if (lastHelloNs != Long.MIN_VALUE) {
                        val rtt = tRecvNs - lastHelloNs
                        if (rtt in 0L until HELLO_PERIOD_NS) pairRttNs = rtt
                    }
                    sourceId = m.sourceId
                    limb = m.limb
                    enter(ConnState.CONNECTED, tRecvNs)
                }
                ConnState.CONNECTED -> {
                    // Reenvío de WELCOME (reintento de HELLO o el juego cambió la extremidad).
                    lastHeardNs = tRecvNs
                    sourceId = m.sourceId
                    limb = m.limb
                }
                ConnState.DISCOVERING, ConnState.REJECTED -> {} // respuesta vieja
            }
        } else {
            when (state) {
                ConnState.DISCOVERING -> {
                    // Respuesta a DISCOVER: el PC se da a conocer (accepted=0, source_id=0).
                    discovered = Endpoint(fromHost, fromPort)
                }
                ConnState.PAIRING, ConnState.LOST -> {
                    lastHeardNs = tRecvNs
                    val echo = lastDiscoverNs != Long.MIN_VALUE && tRecvNs - lastDiscoverNs < DISCOVER_ECHO_GUARD_NS
                    if (!echo) enter(ConnState.REJECTED, tRecvNs)
                }
                ConnState.CONNECTED, ConnState.REJECTED -> {}
            }
        }
    }

    // ------------------------------------------------------------------ muestras

    /** Muestra combinada lista para enviar (ya en base elapsedRealtimeNanos). */
    fun onSample(s: PhoneSample, nowNs: Long) {
        lastNowNs = nowNs
        rate.onEvent(nowNs)
        if (state != ConnState.CONNECTED) return
        batch.add(s)
        if (batch.size >= MAX_BATCH) flush(nowNs)
    }

    private fun flush(nowNs: Long) {
        lastFlushNs = nowNs
        if (batch.isEmpty()) return
        val t = target
        val v = ArrayList(batch)
        batch.clear()
        if (t == null) return
        if (send(Message.Samples(v), t)) samplesSent += v.size else dropped += v.size
    }

    // ------------------------------------------------------------------ temporizadores

    /** Llamar a menudo (≤ 2-5 ms) desde el hilo de red. */
    fun tick(nowNs: Long) {
        lastNowNs = nowNs
        rate.update(nowNs)
        when (state) {
            ConnState.DISCOVERING -> {
                if (lastDiscoverNs == Long.MIN_VALUE || nowNs - lastDiscoverNs >= DISCOVER_PERIOD_NS) {
                    lastDiscoverNs = nowNs
                    send(Message.Discover, null)
                }
            }
            ConnState.PAIRING, ConnState.LOST -> {
                if (lastHelloNs == Long.MIN_VALUE || nowNs - lastHelloNs >= HELLO_PERIOD_NS) sendHello(nowNs)
            }
            ConnState.CONNECTED -> {
                if (lastHeardNs != Long.MIN_VALUE && nowNs - lastHeardNs > TIMEOUT_NS) {
                    // 3 s sin oír al PC: "Sin conexión" y volver a emparejar (re-HELLO).
                    enter(ConnState.LOST, nowNs)
                    sendHello(nowNs)
                    return
                }
                if (batch.isNotEmpty() && (lastFlushNs == Long.MIN_VALUE || nowNs - lastFlushNs >= BATCH_PERIOD_NS)) {
                    flush(nowNs)
                }
                if (nextStatusNs == Long.MIN_VALUE || nowNs >= nextStatusNs) {
                    nextStatusNs = nowNs + STATUS_PERIOD_NS
                    sendStatus()
                }
            }
            ConnState.REJECTED -> {}
        }
    }

    private fun sendStatus() {
        val t = target ?: return
        var bat = batteryProvider()
        if (!(bat >= 0f)) bat = -1f else if (bat > 1f) bat = 1f // NaN o negativo -> desconocido
        val hz = Math.round(rate.rateHz).coerceIn(0, 0xFFFF)
        val dr = dropped.coerceIn(0L, 0xFFFFL).toInt()
        send(Message.Status(bat, hz, dr), t)
    }

    fun snapshot(nowNs: Long = lastNowNs): ClientSnapshot = ClientSnapshot(
        state = state,
        target = target,
        discovered = discovered,
        sourceId = sourceId,
        limb = limb,
        noResponse = state == ConnState.PAIRING && nowNs - maxOf(pairingStartNs, lastHeardNs) > TIMEOUT_NS,
        sampleRateHz = rate.rateHz,
        pairRttMs = if (pairRttNs < 0) -1f else pairRttNs / 1e6f,
        packetsSent = packetsSent,
        samplesSent = samplesSent,
        syncAnswered = syncAnswered,
        dropped = dropped,
        sinceHeardMs = if (lastHeardNs == Long.MIN_VALUE) -1L else (nowNs - lastHeardNs) / 1_000_000L,
    )
}
