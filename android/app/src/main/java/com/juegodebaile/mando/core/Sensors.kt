package com.juegodebaile.mando.core

// Lógica pura del lado sensores: corrección de base de tiempo, limitador a 200 Hz y combinador
// acelerómetro + giroscopio. Sin android.*: la Activity le pasa los valores de SensorEvent.

/**
 * Corrige `SensorEvent.timestamp` a la base de `SystemClock.elapsedRealtimeNanos()`.
 *
 * Contrato: "SensorEvent.timestamp suele usar esa base; si al recibirlo difiere más de 1 s de
 * elapsedRealtimeNanos(), la app corrige con la diferencia medida".
 *
 * La diferencia medida d = now − timestamp = desfase_de_base + latencia_de_entrega (≥ 0), así que el
 * mejor estimador del desfase es el mínimo de d observado: con él las marcas corregidas nunca quedan
 * en el futuro. Si la base vuelve a cambiar (d se aleja > 1 s del desfase actual) se re-mide.
 */
class TimeBaseCorrector(private val thresholdNs: Long = 1_000_000_000L) {
    /** Desfase que se suma a timestamp (0 = misma base). */
    var offsetNs: Long = 0L
        private set
    /** true si se está aplicando una corrección (la base del sensor no es elapsedRealtimeNanos). */
    var correcting: Boolean = false
        private set

    fun correct(eventTimestampNs: Long, nowElapsedNs: Long): Long {
        val d = nowElapsedNs - eventTimestampNs
        if (!correcting) {
            if (d > thresholdNs || d < -thresholdNs) {
                correcting = true
                offsetNs = d
            }
        } else {
            val dev = d - offsetNs
            if (dev > thresholdNs || dev < -thresholdNs) {
                // La base del sensor ha vuelto a cambiar: re-medir (o dejar de corregir).
                if (d > thresholdNs || d < -thresholdNs) {
                    offsetNs = d
                } else {
                    correcting = false
                    offsetNs = 0L
                }
            } else if (d < offsetNs) {
                offsetNs = d // menor latencia observada = mejor estimación
            }
        }
        return eventTimestampNs + offsetNs
    }
}

/**
 * Limita la cadencia de muestras a `periodNs` (200 Hz) aunque el sensor entregue más rápido
 * (SENSOR_DELAY_FASTEST / samplingPeriodUs 5000 es una petición, no un límite).
 * Programa "huecos" cada periodNs y acepta una muestra si llega como pronto `toleranceNs` antes de
 * su hueco: a 200 Hz con jitter no pierde muestras; a 400 Hz deja pasar ~1 de cada 2.
 */
class RateLimiter(
    private val periodNs: Long = 5_000_000L,
    private val toleranceNs: Long = 2_000_000L,
) {
    private var nextNs: Long = Long.MIN_VALUE

    fun accept(tNs: Long): Boolean {
        if (nextNs == Long.MIN_VALUE) {
            nextNs = tNs + periodNs
            return true
        }
        if (tNs < nextNs - toleranceNs) return false
        // Si nos hemos quedado muy atrás (pausa del sensor) se re-ancla al instante actual.
        nextNs = if (tNs - nextNs > periodNs) tNs + periodNs else nextNs + periodNs
        return true
    }

    fun reset() {
        nextNs = Long.MIN_VALUE
    }
}

/**
 * Combina eventos separados de acelerómetro y giroscopio (Android los entrega por separado).
 * Cada muestra de acelerómetro se empareja con la ÚLTIMA de giroscopio; si |Δt| ≤ 5 ms la muestra
 * lleva ambos flags; si no (o no hay giroscopio), solo HAS_ACCEL y gyro = 0.
 * Las marcas de tiempo que recibe ya deben estar corregidas (TimeBaseCorrector).
 */
class SampleCombiner(
    private val maxGyroDtNs: Long = 5_000_000L,
    private val limiter: RateLimiter? = RateLimiter(),
) {
    private var gyroT: Long = Long.MIN_VALUE
    private var gx = 0f
    private var gy = 0f
    private var gz = 0f
    private var lastOutT: Long = Long.MIN_VALUE

    fun onGyro(tNs: Long, x: Float, y: Float, z: Float) {
        gyroT = tNs
        gx = x; gy = y; gz = z
    }

    /** Devuelve la muestra combinada, o null si se descarta (limitador o marca no creciente). */
    fun onAccel(tNs: Long, x: Float, y: Float, z: Float): PhoneSample? {
        if (tNs <= lastOutT) return null // duplicada / desordenada: el PC también la descartaría
        if (limiter != null && !limiter.accept(tNs)) return null
        lastOutT = tNs
        val haveGyro = gyroT != Long.MIN_VALUE && absDiff(tNs, gyroT) <= maxGyroDtNs
        return if (haveGyro) {
            PhoneSample(tNs, Protocol.FLAG_HAS_ACCEL or Protocol.FLAG_HAS_GYRO, x, y, z, gx, gy, gz)
        } else {
            PhoneSample(tNs, Protocol.FLAG_HAS_ACCEL, x, y, z, 0f, 0f, 0f)
        }
    }

    fun reset() {
        gyroT = Long.MIN_VALUE
        lastOutT = Long.MIN_VALUE
        limiter?.reset()
    }

    private fun absDiff(a: Long, b: Long): Long = if (a >= b) a - b else b - a
}

/** Mide la tasa real (eventos/s) en ventanas de ≥ 1 s. */
class RateMeter(private val windowNs: Long = 1_000_000_000L) {
    private var start: Long = Long.MIN_VALUE
    private var count: Long = 0
    /** Última tasa medida (Hz); 0 si aún no hay ventana completa. */
    @Volatile
    var rateHz: Float = 0f
        private set

    fun onEvent(nowNs: Long) {
        update(nowNs)
        count++
    }

    /** Cierra la ventana si ha pasado su duración (llamar también sin eventos para que caiga a 0). */
    fun update(nowNs: Long) {
        if (start == Long.MIN_VALUE) {
            start = nowNs
            count = 0
            return
        }
        val dt = nowNs - start
        if (dt >= windowNs) {
            rateHz = (count.toDouble() * 1e9 / dt.toDouble()).toFloat()
            start = nowNs
            count = 0
        }
    }

    fun reset() {
        start = Long.MIN_VALUE
        count = 0
        rateHz = 0f
    }
}
