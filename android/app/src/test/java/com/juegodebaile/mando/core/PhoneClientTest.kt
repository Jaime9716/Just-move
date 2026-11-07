package com.juegodebaile.mando.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PhoneClientTest {
    private val ms = 1_000_000L
    private val dev = 0x1234ABCD
    private val pc = Endpoint("192.168.1.20", 47800)

    private class Sent(val packet: Packet, val dest: Endpoint?, val bport: Int)

    private class Rig(deviceId: Int, battery: Float = 0.5f) {
        var now = 1_000L * 1_000_000L
        val sent = ArrayList<Sent>()
        var failSends = false
        val client = PhoneClient(
            deviceId = deviceId,
            deviceName = "Pixel 8",
            clock = MonoClock { now },
            sender = Sender { bytes, dest, bport ->
                if (failSends) false else {
                    sent.add(Sent(Protocol.decode(bytes)!!, dest, bport)); true
                }
            },
            batteryProvider = { battery },
        )

        fun fromPc(deviceId: Int, msg: Message, host: String = "192.168.1.20", port: Int = 47800, seq: Int = 1) {
            val b = Protocol.encode(Packet(deviceId, seq, msg))
            // Con desplazamiento dentro de un búfer mayor, como DatagramPacket.
            val buf = ByteArray(b.size + 7)
            System.arraycopy(b, 0, buf, 3, b.size)
            client.onDatagram(buf, 3, b.size, host, port, now)
        }

        /** Avanza el reloj en pasos de 1 ms llamando a tick. */
        fun advance(totalNs: Long, stepNs: Long = 1_000_000L) {
            val end = now + totalNs
            while (now < end) { now += stepNs; client.tick(now) }
        }

        fun of(type: Class<*>) = sent.filter { type.isInstance(it.packet.msg) }
        fun clear() = sent.clear()
    }

    private fun connected(): Rig {
        val r = Rig(dev)
        r.client.connect(pc, 4821, r.now)
        r.now += 3 * ms
        r.fromPc(dev, Message.Welcome(1, Protocol.LIMB_RIGHT_HAND, true))
        assertEquals(ConnState.CONNECTED, r.client.state)
        r.clear()
        return r
    }

    @Test
    fun descubreElPcPorBroadcast() {
        val r = Rig(dev)
        assertEquals(ConnState.DISCOVERING, r.client.state)
        r.client.tick(r.now)
        r.advance(2_500 * ms)
        val disc = r.of(Message.Discover::class.java)
        assertEquals(3, disc.size) // t=0, 1 s, 2 s
        assertTrue(disc.all { it.dest == null && it.bport == 47800 && it.packet.deviceId == dev })
        // El PC responde con WELCOME accepted=0, source_id=0 (server.rs).
        r.fromPc(dev, Message.Welcome(0, Protocol.LIMB_NONE, false), host = "192.168.1.33", port = 47800)
        assertEquals(Endpoint("192.168.1.33", 47800), r.client.discovered)
        assertEquals(ConnState.DISCOVERING, r.client.state)
        assertEquals(Endpoint("192.168.1.33", 47800), r.client.snapshot().discovered)
    }

    @Test
    fun helloCada500msHastaWelcome() {
        val r = Rig(dev)
        r.client.connect(pc, 4821, r.now)
        assertEquals(ConnState.PAIRING, r.client.state)
        val first = r.sent.single()
        assertEquals(Message.Hello(4821, "Pixel 8"), first.packet.msg)
        assertEquals(pc, first.dest)
        r.advance(1_600 * ms)
        val hellos = r.of(Message.Hello::class.java)
        assertEquals(4, hellos.size) // 0, 500, 1000, 1500 ms
        assertTrue(r.of(Message.Discover::class.java).isEmpty())
        assertTrue(r.of(Message.Samples::class.java).isEmpty())
        // Más de 3 s sin respuesta: la UI muestra "Sin conexión" pero sigue reintentando.
        r.advance(1_500 * ms)
        assertTrue(r.client.snapshot(r.now).noResponse)
        r.now += 2 * ms
        r.fromPc(dev, Message.Welcome(1, Protocol.LIMB_RIGHT_HAND, true))
        assertEquals(ConnState.CONNECTED, r.client.state)
        assertEquals(1, r.client.sourceId)
        assertEquals(Protocol.LIMB_RIGHT_HAND, r.client.limb)
        val snap = r.client.snapshot(r.now)
        assertFalse(snap.noResponse)
        assertTrue(snap.pairRttMs in 0f..500f)
        val before = r.of(Message.Hello::class.java).size
        r.advance(1_000 * ms)
        assertEquals(before, r.of(Message.Hello::class.java).size) // conectado: ya no hay HELLO
    }

    @Test
    fun codigoIncorrecto() {
        val r = Rig(dev)
        r.client.connect(pc, 1111, r.now)
        r.now += 600 * ms // lejos de cualquier DISCOVER
        r.fromPc(dev, Message.Welcome(0, Protocol.LIMB_NONE, false))
        assertEquals(ConnState.REJECTED, r.client.state)
        r.clear()
        r.advance(2_000 * ms)
        assertTrue(r.sent.isEmpty()) // no insiste con el código malo
        r.client.connect(pc, 4821, r.now)
        assertEquals(ConnState.PAIRING, r.client.state)
        assertEquals(Message.Hello(4821, "Pixel 8"), r.sent.last().packet.msg)
    }

    @Test
    fun respuestaTardiaADiscoverNoEsRechazo() {
        val r = Rig(dev)
        r.client.tick(r.now) // DISCOVER
        r.now += 20 * ms
        r.client.connect(pc, 4821, r.now)
        r.now += 30 * ms
        r.fromPc(dev, Message.Welcome(0, Protocol.LIMB_NONE, false)) // eco del DISCOVER
        assertEquals(ConnState.PAIRING, r.client.state)
    }

    @Test
    fun ignoraPaquetesDeOtroDispositivoYBasura() {
        val r = Rig(dev)
        r.client.connect(pc, 4821, r.now)
        r.fromPc(dev + 1, Message.Welcome(1, 1, true))
        assertEquals(ConnState.PAIRING, r.client.state)
        r.client.onDatagram(ByteArray(40) { 7 }, 0, 40, "x", 1, r.now)
        r.client.onDatagram(ByteArray(3), 0, 3, "x", 1, r.now)
        assertEquals(ConnState.PAIRING, r.client.state)
    }

    @Test
    fun respondeSyncReqAlInstante() {
        val r = connected()
        val tRecv = r.now
        val b = Protocol.encode(Packet(dev, 5, Message.SyncReq(777_000_000_000L)))
        r.now += 37_000 // 37 µs entre recepción y envío
        r.client.onDatagram(b, 0, b.size, "192.168.1.20", 47800, tRecv)
        val resp = r.sent.single()
        assertEquals(pc, resp.dest)
        assertEquals(Message.SyncResp(777_000_000_000L, tRecv, tRecv + 37_000), resp.packet.msg)
        assertEquals(1L, r.client.syncAnswered)
    }

    @Test
    fun lotesDeHasta8MuestrasCada10ms() {
        val r = connected()
        val produced = ArrayList<Long>()
        // 1 s de muestras a 200 Hz con tick cada 1 ms.
        for (step in 0 until 1000) {
            r.now += ms
            if (step % 5 == 0) {
                val s = PhoneSample(r.now, 3, 0f, 9.8f, 0f, 0f, 0f, 0f)
                produced.add(s.tNs)
                r.client.onSample(s, r.now)
            }
            r.client.tick(r.now)
        }
        val batches = r.of(Message.Samples::class.java).map { it.packet.msg as Message.Samples }
        assertTrue(batches.all { it.samples.size in 1..8 })
        assertTrue("paquetes=${batches.size}", batches.size in 90..110) // ~uno cada 10 ms
        val sentT = batches.flatMap { b -> b.samples.map { it.tNs } }
        assertTrue(produced.size - sentT.size <= 2) // a lo sumo las del último lote pendiente
        assertEquals(produced.subList(0, sentT.size), sentT)
        // Una ráfaga (p. ej. el hilo de red se retrasó) se parte en lotes de 8.
        r.now += 11 * ms
        r.client.tick(r.now) // vacía el lote pendiente
        r.clear()
        for (k in 0 until 20) r.client.onSample(PhoneSample(r.now + k, 3, 0f, 0f, 0f, 0f, 0f, 0f), r.now)
        r.now += 11 * ms
        r.client.tick(r.now)
        val sizes = r.of(Message.Samples::class.java).map { (it.packet.msg as Message.Samples).samples.size }
        assertEquals(listOf(8, 8, 4), sizes)
    }

    @Test
    fun seqCrecienteYConDesbordamiento() {
        val r = connected()
        for (step in 0 until 300) {
            r.now += ms
            if (step % 5 == 0) r.client.onSample(PhoneSample(r.now, 1, 0f, 0f, 0f, 0f, 0f, 0f), r.now)
            r.client.tick(r.now)
        }
        val seqs = r.sent.map { it.packet.seq.toLong() and 0xFFFFFFFFL }
        for (k in 1 until seqs.size) assertEquals(seqs[k - 1] + 1, seqs[k])
        assertEquals(r.client.lastSeq.toLong() and 0xFFFFFFFFL, seqs.last())
    }

    @Test
    fun statusCada2sConBateriaYTasa() {
        val r = connected()
        for (step in 0 until 4_500) {
            r.now += ms
            if (step % 5 == 0) r.client.onSample(PhoneSample(r.now, 3, 0f, 0f, 0f, 0f, 0f, 0f), r.now)
            if (step % 1000 == 0) r.fromPc(dev, Message.SyncReq(r.now)) // el PC sigue vivo
            r.client.tick(r.now)
        }
        val st = r.of(Message.Status::class.java).map { it.packet.msg as Message.Status }
        assertEquals(3, st.size) // al conectar, +2 s, +4 s
        assertEquals(0.5f, st[0].battery)
        assertEquals(200, st.last().sensorRateHz)
        assertEquals(0, st.last().dropped)
        assertEquals(200f, r.client.snapshot(r.now).sampleRateHz, 2f)
    }

    @Test
    fun bateriaDesconocida() {
        val r = Rig(dev, battery = Float.NaN)
        r.client.connect(pc, 1, r.now)
        r.fromPc(dev, Message.Welcome(1, 1, true))
        r.client.tick(r.now)
        assertEquals(-1f, (r.of(Message.Status::class.java).single().packet.msg as Message.Status).battery)
    }

    @Test
    fun reHelloTras3sSinOirAlPc() {
        val r = connected()
        r.advance(2_900 * ms)
        assertEquals(ConnState.CONNECTED, r.client.state)
        r.advance(200 * ms)
        assertEquals(ConnState.LOST, r.client.state)
        val h1 = r.of(Message.Hello::class.java).size
        assertEquals(1, h1)
        r.client.onSample(PhoneSample(r.now, 3, 0f, 0f, 0f, 0f, 0f, 0f), r.now)
        r.advance(1_050 * ms)
        assertEquals(3, r.of(Message.Hello::class.java).size) // cada 500 ms
        assertTrue(r.of(Message.Samples::class.java).isEmpty()) // sin WELCOME no se envían muestras
        r.fromPc(dev, Message.Welcome(1, Protocol.LIMB_RIGHT_HAND, true))
        assertEquals(ConnState.CONNECTED, r.client.state)
    }

    @Test
    fun syncReqMantieneViva() {
        val r = connected()
        for (k in 0 until 10) {
            r.advance(1_000 * ms)
            r.fromPc(dev, Message.SyncReq(r.now))
        }
        assertEquals(ConnState.CONNECTED, r.client.state)
    }

    @Test
    fun byeDelPcPasaASinConexion() {
        val r = connected()
        r.fromPc(dev, Message.Bye)
        assertEquals(ConnState.LOST, r.client.state)
        r.advance(600 * ms)
        assertTrue(r.of(Message.Hello::class.java).isNotEmpty())
    }

    @Test
    fun desconectarEnviaBye() {
        val r = connected()
        r.client.disconnect(r.now)
        assertEquals(Message.Bye, r.sent.last().packet.msg)
        assertEquals(pc, r.sent.last().dest)
        assertEquals(ConnState.DISCOVERING, r.client.state)
        r.clear()
        r.client.tick(r.now)
        assertEquals(Message.Discover, r.sent.single().packet.msg)
        r.client.onSample(PhoneSample(r.now, 3, 0f, 0f, 0f, 0f, 0f, 0f), r.now)
        r.advance(50 * ms)
        assertTrue(r.of(Message.Samples::class.java).isEmpty())
    }

    @Test
    fun envioFallidoCuentaComoPerdida() {
        val r = connected()
        r.failSends = true
        for (k in 0 until 8) r.client.onSample(PhoneSample(r.now + k, 3, 0f, 0f, 0f, 0f, 0f, 0f), r.now)
        assertEquals(8L, r.client.dropped)
        r.client.noteDropped(5)
        assertEquals(13L, r.client.dropped)
        r.failSends = false
        r.advance(2_100 * ms)
        // (el PC calla, pero antes de 3 s aún se envía STATUS con dropped)
        val st = r.of(Message.Status::class.java).map { it.packet.msg as Message.Status }
        assertEquals(13, st.last().dropped)
    }

    @Test
    fun seqDesbordaComoU32() {
        val r = Rig(dev)
        val f = PhoneClient::class.java.getDeclaredField("seq")
        f.isAccessible = true
        f.setInt(r.client, -2) // 0xFFFFFFFE
        r.client.tick(r.now)
        r.client.connect(pc, 1, r.now)
        r.client.disconnect(r.now)
        val seqs = r.sent.map { it.packet.seq }
        assertEquals(listOf(-1, 0, 1), seqs) // 0xFFFFFFFF, 0, 1
    }
}
