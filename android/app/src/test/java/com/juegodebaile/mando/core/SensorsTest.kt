package com.juegodebaile.mando.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SensorsTest {
    private val ms = 1_000_000L

    @Test
    fun combinaAccelConGyroCercano() {
        val c = SampleCombiner(limiter = null)
        c.onGyro(100 * ms, 0.1f, 0.2f, 0.3f)
        val s = c.onAccel(104 * ms, 1f, 2f, 3f)!!
        assertEquals(Protocol.FLAG_HAS_ACCEL or Protocol.FLAG_HAS_GYRO, s.flags)
        assertEquals(104 * ms, s.tNs)
        assertEquals(1f, s.ax); assertEquals(3f, s.az)
        assertEquals(0.1f, s.gx); assertEquals(0.3f, s.gz)
    }

    @Test
    fun limiteDeCincoMilisegundos() {
        val c = SampleCombiner(limiter = null)
        c.onGyro(100 * ms, 1f, 1f, 1f)
        assertEquals(3, c.onAccel(105 * ms, 0f, 0f, 9.8f)!!.flags) // |Δt| = 5 ms: vale
        val s = c.onAccel(105 * ms + 1, 0f, 0f, 9.8f)!! // 5 ms + 1 ns: no vale
        assertEquals(Protocol.FLAG_HAS_ACCEL, s.flags)
        assertEquals(0f, s.gx); assertEquals(0f, s.gy); assertEquals(0f, s.gz)
        // Giroscopio "del futuro" (llega antes con marca posterior) también cuenta por |Δt|.
        c.onGyro(210 * ms, 2f, 2f, 2f)
        assertEquals(3, c.onAccel(206 * ms, 0f, 0f, 9.8f)!!.flags)
    }

    @Test
    fun sinGiroscopioSoloAccel() {
        val c = SampleCombiner(limiter = null)
        assertEquals(Protocol.FLAG_HAS_ACCEL, c.onAccel(1 * ms, 0f, 9.8f, 0f)!!.flags)
    }

    @Test
    fun descartaMarcasNoCrecientes() {
        val c = SampleCombiner(limiter = null)
        assertNotNull(c.onAccel(10 * ms, 0f, 0f, 0f))
        assertNull(c.onAccel(10 * ms, 0f, 0f, 0f))
        assertNull(c.onAccel(9 * ms, 0f, 0f, 0f))
        assertNotNull(c.onAccel(11 * ms, 0f, 0f, 0f))
        c.reset()
        assertNotNull(c.onAccel(1 * ms, 0f, 0f, 0f))
    }

    @Test
    fun limitadorA200Hz() {
        val l = RateLimiter()
        var accepted = 0
        // Sensor a 400 Hz durante 1 s -> ~200 aceptadas.
        for (k in 0 until 400) if (l.accept(k * 2_500_000L)) accepted++
        assertTrue("aceptadas=$accepted", accepted in 195..205)
        // A 200 Hz con jitter de ±1 ms no se pierde ninguna.
        val l2 = RateLimiter()
        var acc2 = 0
        for (k in 0 until 200) {
            val jitter = if (k % 2 == 0) 1 * ms else -1 * ms
            if (l2.accept(k * 5 * ms + jitter)) acc2++
        }
        assertEquals(200, acc2)
    }

    @Test
    fun limitadorReanclaTrasPausa() {
        val l = RateLimiter()
        assertTrue(l.accept(0))
        assertTrue(l.accept(5 * ms))
        assertTrue(l.accept(1000 * ms)) // pausa larga
        assertFalse(l.accept(1001 * ms))
        assertTrue(l.accept(1005 * ms))
    }

    @Test
    fun baseDeTiempoIgualNoSeCorrige() {
        val t = TimeBaseCorrector()
        assertEquals(5_000 * ms, t.correct(5_000 * ms, 5_002 * ms))
        assertFalse(t.correcting)
        // 900 ms de retraso: por debajo del umbral de 1 s, se deja tal cual.
        assertEquals(5_100 * ms, t.correct(5_100 * ms, 6_000 * ms))
        assertFalse(t.correcting)
    }

    @Test
    fun baseDistintaSeCorrigeConLaDiferenciaMedida() {
        val t = TimeBaseCorrector()
        val base = 3_600_000 * ms // el sensor va 1 h por detrás (p. ej. base uptimeMillis sin sueño)
        val out1 = t.correct(10_000 * ms, 10_000 * ms + base + 3 * ms)
        assertTrue(t.correcting)
        assertEquals(10_003 * ms + base, out1)
        // Una entrega con menos latencia mejora la estimación (nunca marcas en el futuro).
        val out2 = t.correct(10_010 * ms, 10_010 * ms + base + 1 * ms)
        assertEquals(10_011 * ms + base, out2)
        assertEquals(base + 1 * ms, t.offsetNs)
        // Más latencia no empeora el desfase.
        assertEquals(10_021 * ms + base, t.correct(10_020 * ms, 10_020 * ms + base + 4 * ms))
        // Base del sensor en el futuro (diferencia negativa) también se corrige.
        val f = TimeBaseCorrector()
        assertEquals(2_000 * ms, f.correct(2_000 * ms + 5_000 * ms, 2_000 * ms))
        assertTrue(f.correcting)
    }

    @Test
    fun baseVuelveANormal() {
        val t = TimeBaseCorrector()
        t.correct(0, 5_000 * ms)
        assertTrue(t.correcting)
        assertEquals(9_000 * ms, t.correct(9_000 * ms, 9_001 * ms))
        assertFalse(t.correcting)
    }

    @Test
    fun medidorDeTasa() {
        val r = RateMeter()
        for (k in 0..200) r.onEvent(k * 5 * ms)
        assertEquals(200f, r.rateHz, 1.5f)
        r.update(3_000 * ms) // cierra la ventana con 1 evento en 2 s
        r.update(4_500 * ms) // ventana vacía
        assertEquals(0f, r.rateHz, 0.01f)
    }
}
