package com.juegodebaile.mando

import android.app.Activity
import android.content.Context
import android.content.pm.ActivityInfo
import android.graphics.Color
import android.graphics.Typeface
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.InputFilter
import android.text.InputType
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.WindowManager
import android.view.inputmethod.InputMethodManager
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import com.juegodebaile.mando.core.ClientSnapshot
import com.juegodebaile.mando.core.ConnState
import com.juegodebaile.mando.core.Endpoint
import com.juegodebaile.mando.core.Protocol
import com.juegodebaile.mando.core.parsePairingCode
import java.security.SecureRandom
import java.util.Locale

/**
 * Pantalla única del mando (Views creadas por código, sin AndroidX).
 * El trabajo de verdad lo hace [MandoController] en sus propios hilos; aquí solo se pinta su estado
 * cada 200 ms y se le pasan las órdenes Conectar/Desconectar.
 */
class MainActivity : Activity() {
    companion object {
        private const val PREFS = "mando"
        private const val KEY_DEVICE_ID = "device_id"
        private const val KEY_HOST = "host"
        private const val KEY_CODE = "code"
        private const val UI_PERIOD_MS = 200L
        private const val COLOR_BG = 0xFF14101C.toInt()
        private const val COLOR_TEXT = 0xFFEDE7F6.toInt()
        private const val COLOR_DIM = 0xFFB0A8C0.toInt()
        private const val COLOR_SEARCH = 0xFFFFC107.toInt()
        private const val COLOR_OK = 0xFF4CAF50.toInt()
        private const val COLOR_BAD = 0xFFF44336.toInt()
    }

    private lateinit var controller: MandoController
    private lateinit var hostEdit: EditText
    private lateinit var codeEdit: EditText
    private lateinit var connectButton: Button
    private lateinit var statusText: TextView
    private lateinit var detailText: TextView

    private val ui = Handler(Looper.getMainLooper())
    /** El usuario pulsó Conectar (y no Desconectar): al volver a primer plano se reconecta. */
    private var wantConnected = false
    private var lastEndpoint: Endpoint? = null
    private var lastCode = 0
    /** Último valor que escribió el autorrelleno (si el usuario lo cambia, no se vuelve a pisar). */
    private var autoFilledHost: String? = null

    private val refresh = object : Runnable {
        override fun run() {
            render()
            ui.postDelayed(this, UI_PERIOD_MS)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_PORTRAIT
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        controller = MandoController(this, loadDeviceId())
        setContentView(buildUi())
        val prefs = getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        hostEdit.setText(prefs.getString(KEY_HOST, "") ?: "")
        codeEdit.setText(prefs.getString(KEY_CODE, "") ?: "")
    }

    override fun onStart() {
        super.onStart()
        controller.start()
        val ep = lastEndpoint
        if (wantConnected && ep != null) controller.connect(ep, lastCode)
        ui.post(refresh)
    }

    override fun onStop() {
        ui.removeCallbacks(refresh)
        // En segundo plano Android deja de entregar sensores: se envía BYE y se para todo.
        controller.stop()
        super.onStop()
    }

    // ------------------------------------------------------------------ device_id persistente

    private fun loadDeviceId(): Int {
        val prefs = getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        if (prefs.contains(KEY_DEVICE_ID)) {
            val id = prefs.getInt(KEY_DEVICE_ID, 0)
            if (id != 0) return id
        }
        val rnd = SecureRandom()
        var id = 0
        while (id == 0) id = rnd.nextInt()
        prefs.edit().putInt(KEY_DEVICE_ID, id).apply()
        return id
    }

    // ------------------------------------------------------------------ UI

    private fun dp(v: Int): Int =
        TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, v.toFloat(), resources.displayMetrics).toInt()

    private fun label(text: String, sizeSp: Float, color: Int): TextView {
        val t = TextView(this)
        t.text = text
        t.setTextSize(TypedValue.COMPLEX_UNIT_SP, sizeSp)
        t.setTextColor(color)
        return t
    }

    private fun lp(topDp: Int): LinearLayout.LayoutParams {
        val p = LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT,
            LinearLayout.LayoutParams.WRAP_CONTENT,
        )
        p.topMargin = dp(topDp)
        return p
    }

    private fun buildUi(): View {
        val col = LinearLayout(this)
        col.orientation = LinearLayout.VERTICAL
        col.setPadding(dp(20), dp(24), dp(20), dp(24))

        val title = label("Mando · Juego de Baile", 24f, COLOR_TEXT)
        title.setTypeface(title.typeface, Typeface.BOLD)
        col.addView(title, lp(0))

        col.addView(
            label(
                "Sujeta el móvil en la mano derecha, pantalla hacia arriba. " +
                    "Escribe la IP:puerto y el código de 4 dígitos que muestra el juego en el PC.",
                16f, COLOR_DIM,
            ),
            lp(8),
        )

        col.addView(label("PC (IP:puerto)", 14f, COLOR_DIM), lp(20))
        hostEdit = EditText(this)
        hostEdit.hint = "192.168.1.20:${Protocol.DEFAULT_PORT}"
        hostEdit.setSingleLine(true)
        hostEdit.inputType = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_URI
        hostEdit.setTextColor(COLOR_TEXT)
        hostEdit.setHintTextColor(COLOR_DIM)
        col.addView(hostEdit, lp(0))

        col.addView(label("Código de emparejado", 14f, COLOR_DIM), lp(12))
        codeEdit = EditText(this)
        codeEdit.hint = "0000"
        codeEdit.setSingleLine(true)
        codeEdit.inputType = InputType.TYPE_CLASS_NUMBER
        codeEdit.filters = arrayOf<InputFilter>(InputFilter.LengthFilter(4))
        codeEdit.setTextColor(COLOR_TEXT)
        codeEdit.setHintTextColor(COLOR_DIM)
        codeEdit.setTextSize(TypedValue.COMPLEX_UNIT_SP, 22f)
        col.addView(codeEdit, lp(0))

        connectButton = Button(this)
        connectButton.text = "Conectar"
        connectButton.setTextSize(TypedValue.COMPLEX_UNIT_SP, 18f)
        connectButton.setOnClickListener { onConnectClicked() }
        col.addView(connectButton, lp(16))

        statusText = label("Buscando PC…", 30f, COLOR_SEARCH)
        statusText.setTypeface(statusText.typeface, Typeface.BOLD)
        statusText.gravity = Gravity.CENTER_HORIZONTAL
        col.addView(statusText, lp(28))

        detailText = label("", 15f, COLOR_DIM)
        detailText.gravity = Gravity.CENTER_HORIZONTAL
        col.addView(detailText, lp(12))

        val scroll = ScrollView(this)
        scroll.setBackgroundColor(COLOR_BG)
        scroll.isFillViewport = true
        scroll.addView(col)
        return scroll
    }

    private fun isActive(s: ClientSnapshot?): Boolean =
        s != null && (s.state == ConnState.PAIRING || s.state == ConnState.CONNECTED || s.state == ConnState.LOST)

    private fun onConnectClicked() {
        if (isActive(controller.snapshot)) {
            wantConnected = false
            controller.disconnect()
            return
        }
        val ep = Endpoint.parse(hostEdit.text.toString())
        if (ep == null) {
            hostEdit.error = "Escribe la IP del PC, p. ej. 192.168.1.20:${Protocol.DEFAULT_PORT}"
            return
        }
        val code = parsePairingCode(codeEdit.text.toString())
        if (code == null) {
            codeEdit.error = "El código tiene 4 dígitos"
            return
        }
        getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
            .putString(KEY_HOST, hostEdit.text.toString().trim())
            .putString(KEY_CODE, codeEdit.text.toString().trim())
            .apply()
        lastEndpoint = ep
        lastCode = code
        wantConnected = true
        controller.connect(ep, code)
        val imm = getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        imm.hideSoftInputFromWindow(connectButton.windowToken, 0)
    }

    private fun render() {
        val s = controller.snapshot
        val err = controller.lastError

        // Autorrelleno con la respuesta a DISCOVER, salvo que el usuario haya escrito otra cosa.
        val disc = s?.discovered
        if (disc != null) {
            val current = hostEdit.text.toString().trim()
            val discText = disc.toString()
            if (current != discText && (current.isEmpty() || current == autoFilledHost)) {
                hostEdit.setText(discText)
                autoFilledHost = discText
            }
        }

        val status: String
        val color: Int
        when {
            s == null -> { status = "Iniciando…"; color = COLOR_SEARCH }
            s.state == ConnState.CONNECTED -> {
                status = "Conectado como ${Protocol.limbNameEs(s.limb)}"; color = COLOR_OK
            }
            s.state == ConnState.REJECTED -> { status = "Código incorrecto"; color = COLOR_BAD }
            s.state == ConnState.LOST -> { status = "Sin conexión"; color = COLOR_BAD }
            s.state == ConnState.PAIRING && s.noResponse -> { status = "Sin conexión"; color = COLOR_BAD }
            s.state == ConnState.PAIRING -> { status = "Buscando PC…"; color = COLOR_SEARCH }
            else -> {
                status = if (disc != null) "Buscando PC… (encontrado $disc)" else "Buscando PC…"
                color = COLOR_SEARCH
            }
        }
        statusText.text = status
        statusText.setTextColor(color)
        connectButton.text = if (isActive(s)) "Desconectar" else "Conectar"

        val sb = StringBuilder()
        if (s != null) {
            sb.append(String.format(Locale.ROOT, "Tasa real: %.0f Hz", s.sampleRateHz))
            sb.append(
                if (s.pairRttMs >= 0f) String.format(Locale.ROOT, "  ·  rtt: %.1f ms", s.pairRttMs)
                else "  ·  rtt: —",
            )
            if (s.state == ConnState.CONNECTED) {
                sb.append("\nPaquetes: ").append(s.packetsSent)
                    .append("  ·  muestras: ").append(s.samplesSent)
                    .append("  ·  perdidas: ").append(s.dropped)
            }
            if (s.state == ConnState.REJECTED) sb.append("\nRevisa el código que muestra el PC y pulsa Conectar.")
            if (s.state == ConnState.LOST) sb.append("\nReintentando cada 0,5 s…")
        }
        if (!controller.hasAccelerometer) sb.append("\nEste móvil no tiene acelerómetro.")
        else if (!controller.hasGyroscope) sb.append("\nSin giroscopio: solo se envía el acelerómetro.")
        if (err.isNotEmpty()) sb.append("\n").append(err)
        detailText.text = sb.toString()
    }
}
