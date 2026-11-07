package com.juegodebaile.mando.core

/** Destino UDP "host:puerto" tal como lo escribe el usuario (sin resolver: lo resuelve la capa Android). */
data class Endpoint(val host: String, val port: Int) {
    override fun toString(): String = if (host.contains(':')) "[$host]:$port" else "$host:$port"

    companion object {
        /**
         * Acepta "192.168.1.20:47800", "192.168.1.20" (puerto por defecto 47800), "pc.local:5000",
         * "[fe80::1]:47800". Devuelve null si está vacío o el puerto no es válido.
         */
        fun parse(text: String, defaultPort: Int = Protocol.DEFAULT_PORT): Endpoint? {
            val s = text.trim()
            if (s.isEmpty()) return null
            val host: String
            val portText: String?
            if (s.startsWith("[")) {
                val close = s.indexOf(']')
                if (close <= 1) return null
                host = s.substring(1, close)
                val rest = s.substring(close + 1)
                portText = when {
                    rest.isEmpty() -> null
                    rest.startsWith(":") -> rest.substring(1)
                    else -> return null
                }
            } else {
                val colons = s.count { it == ':' }
                if (colons == 0) {
                    host = s; portText = null
                } else if (colons == 1) {
                    val i = s.indexOf(':')
                    host = s.substring(0, i); portText = s.substring(i + 1)
                } else {
                    host = s; portText = null // IPv6 sin corchetes
                }
            }
            if (host.isEmpty() || host.any { it.isWhitespace() }) return null
            val port = if (portText == null) defaultPort else (portText.toIntOrNull() ?: return null)
            if (port !in 1..65535) return null
            return Endpoint(host, port)
        }
    }
}

/** Código de emparejado: exactamente 4 dígitos (el PC genera 1000..9999). null si no es válido. */
fun parsePairingCode(text: String): Int? {
    val s = text.trim()
    if (s.length != 4 || !s.all { it in '0'..'9' }) return null
    return s.toInt()
}
