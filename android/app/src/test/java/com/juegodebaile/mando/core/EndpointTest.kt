package com.juegodebaile.mando.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class EndpointTest {
    @Test
    fun parseaFormatosHabituales() {
        assertEquals(Endpoint("192.168.1.20", 47800), Endpoint.parse("192.168.1.20:47800"))
        assertEquals(Endpoint("192.168.1.20", 47800), Endpoint.parse(" 192.168.1.20 "))
        assertEquals(Endpoint("pc.local", 5000), Endpoint.parse("pc.local:5000"))
        assertEquals(Endpoint("fe80::1", 47800), Endpoint.parse("[fe80::1]:47800"))
        assertEquals(Endpoint("fe80::1", 47800), Endpoint.parse("fe80::1"))
        assertEquals("[fe80::1]:47800", Endpoint("fe80::1", 47800).toString())
        assertEquals("10.0.0.2:47800", Endpoint("10.0.0.2", 47800).toString())
    }

    @Test
    fun rechazaInvalidos() {
        assertNull(Endpoint.parse(""))
        assertNull(Endpoint.parse("1.2.3.4:"))
        assertNull(Endpoint.parse("1.2.3.4:0"))
        assertNull(Endpoint.parse("1.2.3.4:70000"))
        assertNull(Endpoint.parse("1.2.3.4:abc"))
        assertNull(Endpoint.parse(":47800"))
        assertNull(Endpoint.parse("a b:1"))
    }

    @Test
    fun codigoDeCuatroDigitos() {
        assertEquals(4821, parsePairingCode("4821"))
        assertEquals(12, parsePairingCode("0012"))
        assertEquals(1000, parsePairingCode(" 1000 "))
        assertNull(parsePairingCode("123"))
        assertNull(parsePairingCode("12345"))
        assertNull(parsePairingCode("12a4"))
        assertNull(parsePairingCode("-123"))
    }
}
