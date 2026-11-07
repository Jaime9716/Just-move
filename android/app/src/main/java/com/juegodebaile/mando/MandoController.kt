package com.juegodebaile.mando

import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.net.wifi.WifiManager
import android.os.BatteryManager
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.os.Process
import android.os.SystemClock
import android.util.Log
import com.juegodebaile.mando.core.ClientSnapshot
import com.juegodebaile.mando.core.Endpoint
import com.juegodebaile.mando.core.MonoClock
import com.juegodebaile.mando.core.PhoneClient
import com.juegodebaile.mando.core.PhoneSample
import com.juegodebaile.mando.core.RateMeter
import com.juegodebaile.mando.core.SampleCombiner
import com.juegodebaile.mando.core.Sender
import com.juegodebaile.mando.core.TimeBaseCorrector
import java.io.IOException
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.Inet4Address
import java.net.InetAddress
import java.net.NetworkInterface
import java.net.SocketTimeoutException
import java.util.concurrent.ArrayBlockingQueue
import java.util.concurrent.ConcurrentLinkedQueue
import java.util.concurrent.atomic.AtomicLong

/**
 * Une sensores (hilo "jd-sensors") y red (hilo "jd-net") con la lógica pura de `core`.
 * Ciclo de vida: start() en onStart de la Activity, stop() en onStop.
 */
class MandoController(context: Context, private val deviceId: Int) {
    companion object {
        private const val TAG = "JdMando"
        /** 5000 µs = 200 Hz: el máximo del contrato (y el máximo sin HIGH_SAMPLING_RATE_SENSORS). */
        const val SAMPLING_PERIOD_US = 5000
        private const val QUEUE_CAPACITY = 512
        private const val SOCKET_TIMEOUT_MS = 2
        private const val SNAPSHOT_PERIOD_NS = 100_000_000L
        private const val BROADCAST_REFRESH_NS = 5_000_000_000L
        private const val RESOLVE_RETRY_NS = 2_000_000_000L
    }

    private val app = context.applicationContext
    private val sensorManager = app.getSystemService(Context.SENSOR_SERVICE) as SensorManager
    private val accel: Sensor? = sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER)
    private val gyro: Sensor? = sensorManager.getDefaultSensor(Sensor.TYPE_GYROSCOPE)
    val hasAccelerometer: Boolean get() = accel != null
    val hasGyroscope: Boolean get() = gyro != null

    private val queue = ArrayBlockingQueue<PhoneSample>(QUEUE_CAPACITY)
    private val queueDropped = AtomicLong(0)
    private val commands = ConcurrentLinkedQueue<Command>()

    /** Última foto del cliente (la lee la UI). */
    @Volatile
    var snapshot: ClientSnapshot? = null
        private set
    /** Último error de red legible ("" si ninguno). */
    @Volatile
    var lastError: String = ""
        private set
    /** Tasa de eventos del acelerómetro tal como llegan (antes del limitador), para diagnóstico. */
    val rawAccelRate = RateMeter()

    private var sensorThread: HandlerThread? = null
    private var netThread: Thread? = null
    @Volatile
    private var running = false
    private var multicastLock: WifiManager.MulticastLock? = null
    private var wifiLock: WifiManager.WifiLock? = null

    private sealed class Command {
        class Connect(val ep: Endpoint, val code: Int) : Command()
        object Disconnect : Command()
    }

    // ------------------------------------------------------------------ ciclo de vida

    fun start() {
        if (running) return
        running = true
        acquireWifiLocks()
        startSensors()
        val t = Thread({ netLoop() }, "jd-net")
        netThread = t
        t.start()
    }

    /** Se despide del PC (BYE), para el hilo de red y los sensores. */
    fun stop() {
        if (!running) return
        commands.add(Command.Disconnect)
        running = false
        netThread?.let {
            try {
                it.join(500)
            } catch (_: InterruptedException) {
                Thread.currentThread().interrupt()
            }
        }
        netThread = null
        stopSensors()
        releaseWifiLocks()
    }

    fun connect(ep: Endpoint, code: Int) {
        commands.add(Command.Connect(ep, code))
    }

    fun disconnect() {
        commands.add(Command.Disconnect)
    }

    // ------------------------------------------------------------------ Wi-Fi

    private fun acquireWifiLocks() {
        try {
            val wm = app.getSystemService(Context.WIFI_SERVICE) as WifiManager
            // Recibir broadcast/multicast en algunos móviles exige MulticastLock (las respuestas a
            // DISCOVER llegan por unicast, pero no cuesta nada mientras la app está en primer plano).
            val ml = wm.createMulticastLock("jd-mando-multicast")
            ml.setReferenceCounted(false)
            ml.acquire()
            multicastLock = ml
            // Evita el ahorro de energía del Wi-Fi, que añade decenas de ms de latencia.
            @Suppress("DEPRECATION")
            val mode = if (Build.VERSION.SDK_INT >= 29) WifiManager.WIFI_MODE_FULL_LOW_LATENCY
            else WifiManager.WIFI_MODE_FULL_HIGH_PERF
            val wl = wm.createWifiLock(mode, "jd-mando-lowlatency")
            wl.setReferenceCounted(false)
            wl.acquire()
            wifiLock = wl
        } catch (e: Exception) {
            Log.w(TAG, "no se pudieron tomar los bloqueos de Wi-Fi", e)
        }
    }

    private fun releaseWifiLocks() {
        try {
            multicastLock?.let { if (it.isHeld) it.release() }
            wifiLock?.let { if (it.isHeld) it.release() }
        } catch (e: Exception) {
            Log.w(TAG, "error al soltar los bloqueos de Wi-Fi", e)
        }
        multicastLock = null
        wifiLock = null
    }

    // ------------------------------------------------------------------ sensores

    private val listener = object : SensorEventListener {
        // Solo se usan desde el hilo "jd-sensors".
        private val accelBase = TimeBaseCorrector()
        private val gyroBase = TimeBaseCorrector()
        private val combiner = SampleCombiner()

        override fun onSensorChanged(e: SensorEvent) {
            val now = SystemClock.elapsedRealtimeNanos()
            val v = e.values
            if (v.size < 3) return
            when (e.sensor.type) {
                Sensor.TYPE_GYROSCOPE -> combiner.onGyro(gyroBase.correct(e.timestamp, now), v[0], v[1], v[2])
                Sensor.TYPE_ACCELEROMETER -> {
                    rawAccelRate.onEvent(now)
                    val s = combiner.onAccel(accelBase.correct(e.timestamp, now), v[0], v[1], v[2]) ?: return
                    if (!queue.offer(s)) queueDropped.incrementAndGet()
                }
            }
        }

        override fun onAccuracyChanged(sensor: Sensor, accuracy: Int) {}
    }

    private fun startSensors() {
        val ht = HandlerThread("jd-sensors", Process.THREAD_PRIORITY_URGENT_DISPLAY)
        ht.start()
        sensorThread = ht
        val h = Handler(ht.looper)
        // samplingPeriodUs = 5000 (200 Hz); maxReportLatencyUs = 0: sin lotes en el hub de sensores.
        accel?.let { sensorManager.registerListener(listener, it, SAMPLING_PERIOD_US, 0, h) }
        gyro?.let { sensorManager.registerListener(listener, it, SAMPLING_PERIOD_US, 0, h) }
    }

    private fun stopSensors() {
        sensorManager.unregisterListener(listener)
        sensorThread?.quitSafely()
        sensorThread = null
        queue.clear()
    }

    // ------------------------------------------------------------------ batería

    private fun batteryLevel(): Float {
        return try {
            val bm = app.getSystemService(Context.BATTERY_SERVICE) as BatteryManager
            val pct = bm.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY)
            if (pct in 0..100) pct / 100f else -1f
        } catch (e: Exception) {
            -1f
        }
    }

    // ------------------------------------------------------------------ red

    private fun deviceName(): String {
        val man = Build.MANUFACTURER ?: ""
        val model = Build.MODEL ?: ""
        val name = if (model.startsWith(man, ignoreCase = true)) model else "$man $model"
        return name.trim().ifEmpty { "Android" }
    }

    /** Ata el socket a la red Wi-Fi (si hay) para que el tráfico LAN no salga por datos móviles. */
    private fun bindToWifi(sock: DatagramSocket) {
        try {
            val cm = app.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
            @Suppress("DEPRECATION")
            val nets = cm.allNetworks
            for (n in nets) {
                val caps = cm.getNetworkCapabilities(n) ?: continue
                if (caps.hasTransport(NetworkCapabilities.TRANSPORT_WIFI)) {
                    n.bindSocket(sock)
                    return
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "no se pudo atar el socket a la Wi-Fi; se usa la red por defecto", e)
        }
    }

    /** 255.255.255.255 más el broadcast dirigido de cada interfaz IPv4 activa. */
    private fun broadcastAddresses(): List<InetAddress> {
        val out = ArrayList<InetAddress>()
        try {
            val ifs = NetworkInterface.getNetworkInterfaces()
            if (ifs != null) {
                for (ni in ifs.toList()) {
                    if (!ni.isUp || ni.isLoopback) continue
                    for (ia in ni.interfaceAddresses) {
                        val b = ia.broadcast ?: continue
                        if (ia.address is Inet4Address && b !in out) out.add(b)
                    }
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "no se pudieron enumerar las interfaces", e)
        }
        try {
            out.add(InetAddress.getByName("255.255.255.255"))
        } catch (_: Exception) {
        }
        return out
    }

    private fun netLoop() {
        try {
            Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_DISPLAY)
        } catch (_: Exception) {
        }
        val sock: DatagramSocket
        try {
            sock = DatagramSocket()
            bindToWifi(sock)
            sock.broadcast = true
            sock.soTimeout = SOCKET_TIMEOUT_MS
        } catch (e: Exception) {
            lastError = "No se pudo abrir el socket UDP: ${e.message}"
            Log.e(TAG, lastError, e)
            running = false
            return
        }

        var bcast: List<InetAddress> = broadcastAddresses()
        var bcastAt = SystemClock.elapsedRealtimeNanos()
        val resolved = HashMap<Endpoint, InetAddress>()
        var failedEp: Endpoint? = null
        var failedAt = 0L

        fun resolve(ep: Endpoint): InetAddress? {
            resolved[ep]?.let { return it }
            val now = SystemClock.elapsedRealtimeNanos()
            if (ep == failedEp && now - failedAt < RESOLVE_RETRY_NS) return null
            return try {
                val a = InetAddress.getByName(ep.host)
                resolved[ep] = a
                a
            } catch (e: Exception) {
                failedEp = ep
                failedAt = now
                lastError = "No se encuentra el host ${ep.host}"
                null
            }
        }

        val sender = Sender { bytes, dest, bport ->
            if (dest == null) {
                var any = false
                for (a in bcast) {
                    try {
                        sock.send(DatagramPacket(bytes, bytes.size, a, bport))
                        any = true
                    } catch (_: IOException) {
                        // Sin Wi-Fi el broadcast falla (ENETUNREACH): se reintenta en el siguiente DISCOVER.
                    }
                }
                any
            } else {
                val addr = resolve(dest)
                if (addr == null) {
                    false
                } else {
                    try {
                        sock.send(DatagramPacket(bytes, bytes.size, addr, dest.port))
                        true
                    } catch (e: IOException) {
                        lastError = "Error de envío UDP: ${e.message}"
                        false
                    }
                }
            }
        }

        val client = PhoneClient(
            deviceId = deviceId,
            deviceName = deviceName(),
            clock = MonoClock { SystemClock.elapsedRealtimeNanos() },
            sender = sender,
            batteryProvider = { batteryLevel() },
        )

        val buf = ByteArray(2048)
        val packet = DatagramPacket(buf, buf.size)
        var lastSnap = Long.MIN_VALUE
        try {
            while (true) {
                // Órdenes de la UI (Disconnect se procesa aunque ya no estemos "running": envía BYE).
                while (true) {
                    val c = commands.poll() ?: break
                    val now = SystemClock.elapsedRealtimeNanos()
                    when (c) {
                        is Command.Connect -> {
                            lastError = ""
                            client.connect(c.ep, c.code, now)
                        }
                        is Command.Disconnect -> client.disconnect(now)
                    }
                }
                if (!running) break

                packet.setLength(buf.size) // receive() acorta la longitud al último datagrama
                try {
                    sock.receive(packet)
                    val tRecv = SystemClock.elapsedRealtimeNanos()
                    val from = packet.address?.hostAddress ?: ""
                    client.onDatagram(buf, packet.offset, packet.length, from, packet.port, tRecv)
                } catch (_: SocketTimeoutException) {
                } catch (e: IOException) {
                    lastError = "Error de recepción UDP: ${e.message}"
                    try {
                        Thread.sleep(5)
                    } catch (_: InterruptedException) {
                    }
                }

                val now = SystemClock.elapsedRealtimeNanos()
                while (true) {
                    val s = queue.poll() ?: break
                    client.onSample(s, now)
                }
                client.noteDropped(queueDropped.getAndSet(0))
                client.tick(now)

                if (now - bcastAt >= BROADCAST_REFRESH_NS) {
                    bcast = broadcastAddresses()
                    bcastAt = now
                }
                if (lastSnap == Long.MIN_VALUE || now - lastSnap >= SNAPSHOT_PERIOD_NS) {
                    lastSnap = now
                    snapshot = client.snapshot(now)
                }
            }
        } catch (e: Exception) {
            lastError = "Error en el hilo de red: ${e.message}"
            Log.e(TAG, lastError, e)
        } finally {
            sock.close()
            snapshot = client.snapshot(SystemClock.elapsedRealtimeNanos())
        }
    }
}
