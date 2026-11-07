package com.juegodebaile.mando.core

// Lector JSON mínimo para los tests (sin dependencias: org.json de Android no existe en tests JVM).
// Los números se guardan como texto (JsonNum) para leer i64 y f32 sin pérdida.

class JsonNum(val raw: String) {
    fun toLong(): Long = raw.toLong()
    fun toInt(): Int = raw.toInt()
    /** u32 del contrato -> Int con los mismos 32 bits. */
    fun toU32Bits(): Int = raw.toLong().toInt()
    fun toFloat(): Float = raw.toFloat()
    override fun toString(): String = raw
}

class MiniJson private constructor(private val s: String) {
    private var i = 0

    companion object {
        fun parse(text: String): Any? {
            val p = MiniJson(text)
            p.ws()
            val v = p.value()
            p.ws()
            if (p.i != p.s.length) throw IllegalArgumentException("basura tras el JSON en ${p.i}")
            return v
        }
    }

    private fun ws() {
        while (i < s.length && s[i].isWhitespace()) i++
    }

    private fun fail(msg: String): Nothing = throw IllegalArgumentException("JSON: $msg en la posición $i")

    private fun value(): Any? {
        if (i >= s.length) fail("fin inesperado")
        return when (val c = s[i]) {
            '{' -> obj()
            '[' -> arr()
            '"' -> str()
            't' -> lit("true", true)
            'f' -> lit("false", false)
            'n' -> lit("null", null)
            else -> if (c == '-' || c in '0'..'9') num() else fail("carácter '$c'")
        }
    }

    private fun lit(word: String, v: Any?): Any? {
        if (!s.startsWith(word, i)) fail("se esperaba $word")
        i += word.length
        return v
    }

    private fun num(): JsonNum {
        val st = i
        while (i < s.length && (s[i] == '-' || s[i] == '+' || s[i] == '.' || s[i] == 'e' || s[i] == 'E' || s[i] in '0'..'9')) i++
        return JsonNum(s.substring(st, i))
    }

    private fun str(): String {
        i++ // "
        val sb = StringBuilder()
        while (true) {
            if (i >= s.length) fail("cadena sin cerrar")
            val c = s[i++]
            when (c) {
                '"' -> return sb.toString()
                '\\' -> {
                    val e = s[i++]
                    when (e) {
                        '"' -> sb.append('"')
                        '\\' -> sb.append('\\')
                        '/' -> sb.append('/')
                        'b' -> sb.append('\b')
                        'f' -> sb.append('\u000C')
                        'n' -> sb.append('\n')
                        'r' -> sb.append('\r')
                        't' -> sb.append('\t')
                        'u' -> {
                            sb.append(s.substring(i, i + 4).toInt(16).toChar())
                            i += 4
                        }
                        else -> fail("escape \\$e")
                    }
                }
                else -> sb.append(c)
            }
        }
    }

    private fun arr(): List<Any?> {
        i++
        val out = ArrayList<Any?>()
        ws()
        if (s[i] == ']') { i++; return out }
        while (true) {
            ws(); out.add(value()); ws()
            when (s[i++]) {
                ',' -> continue
                ']' -> return out
                else -> fail("se esperaba , o ]")
            }
        }
    }

    private fun obj(): Map<String, Any?> {
        i++
        val out = LinkedHashMap<String, Any?>()
        ws()
        if (s[i] == '}') { i++; return out }
        while (true) {
            ws()
            if (s[i] != '"') fail("se esperaba clave")
            val k = str()
            ws()
            if (s[i++] != ':') fail("se esperaba :")
            ws()
            out[k] = value()
            ws()
            when (s[i++]) {
                ',' -> continue
                '}' -> return out
                else -> fail("se esperaba , o }")
            }
        }
    }
}
