# Informe del agente P · App Android de mando

## Qué hice

App Android en `android/` (Kotlin, Gradle Kotlin DSL, sin AndroidX/Compose; única dependencia de test
JUnit 4.13.2). Revisé y completé el intento anterior (esqueleto Gradle + `core/` + `MandoController`)
y añadí lo que faltaba: `MainActivity`, permisos, tests, vectores, script de tests sin SDK y README.

- **Versiones**: Gradle 8.7 (wrapper oficial; el jar coincide por sha256 con el que genera `gradle wrapper`),
  AGP 8.5.2, Kotlin 2.0.21, JDK 17, compileSdk/targetSdk 34, minSdk 26, applicationId
  `com.juegodebaile.mando`. Combinación de la tabla oficial: AGP 8.5 exige Gradle ≥ 8.7; KGP 2.0.21
  soporta AGP ≤ 8.5 y Gradle ≤ 8.8.
- **Lógica pura** (`com.juegodebaile.mando.core`, sin `android.*`): protocolo completo, corrector de
  base de tiempo (> 1 s → corrige con la diferencia mínima medida), limitador a 200 Hz, combinador
  accel + último gyro (|Δt| ≤ 5 ms → ambos flags; si no, solo HAS_ACCEL y gyro 0), máquina de estados
  `DISCOVERING → PAIRING (HELLO/500 ms) → CONNECTED | REJECTED`, `LOST` tras 3 s sin oír al PC o BYE
  del PC (re-HELLO cada 500 ms), SYNC_RESP inmediato (t_phone_send leído justo antes de enviar),
  lotes ≤ 8 cada ~10 ms, STATUS cada 2 s (batería 0..1 o −1, tasa real, perdidas saturadas a u16),
  seq u32 creciente con desbordamiento.
- **Android** (`MandoController`, `MainActivity`): sensores con `samplingPeriodUs = 5000` y
  `maxReportLatencyUs = 0` en un `HandlerThread`; red en hilo propio con `DatagramSocket` (timeout 2 ms),
  atado a la red Wi-Fi; DISCOVER a 255.255.255.255 y al broadcast de cada interfaz; `MulticastLock` y
  `WifiLock` de baja latencia. UI por código: IP:puerto autorrellenado por DISCOVER, código de 4 dígitos,
  Conectar/Desconectar, estado grande (Buscando PC… / Conectado como mano derecha / Código incorrecto /
  Sin conexión), tasa real y rtt (HELLO→WELCOME), instrucción de sujeción, `FLAG_KEEP_SCREEN_ON`,
  vertical. `device_id` aleatorio ≠ 0 persistente en SharedPreferences. Al ir a segundo plano envía BYE;
  al volver se reconecta si el usuario estaba conectado.
- Permisos: INTERNET, ACCESS_NETWORK_STATE, ACCESS_WIFI_STATE, CHANGE_WIFI_MULTICAST_STATE y
  **WAKE_LOCK** (lo exige `WifiLock.acquire()`; es permiso normal).

## Cómo probarlo

- CI: `cd android && ./gradlew assembleDebug testDebugUnitTest` (job `android` de `.github/workflows/ci.yml`).
- Aquí, sin SDK ni red: `android/tools/test_core_offline.sh` → **40 tests JUnit OK** (protocolo,
  vectores, endpoint, sensores, máquina de estados). Usa el compilador de Kotlin 2.0.21 y JUnit que trae
  `/opt/gradle-8.14.3/lib`.

## Verificaciones hechas

1. **Vectores generados con el Rust real**: programa temporal en `/tmp/claude-0/pvec` (path-dep a
   `engine/jd_core`, no está en el repo) que codifica 19 mensajes (los 8 tipos, UTF-8 en HELLO, u32 con
   bit alto, i64 extremos, −0.0, 8 muestras…) con `jd_core::phone::protocol::encode` y vuelca
   `android/app/src/test/resources/protocol_vectors.json`. El codificador Kotlin produce los mismos bytes
   exactos (y comprobé que el test falla si se altera un byte).
2. **Prueba extremo a extremo por UDP real** (temporal, fuera del repo): `PhoneServerSource` de jd_core
   en 127.0.0.1 + `PhoneClient` Kotlin con `DatagramSocket` y reloj desplazado 777 s. Resultado: código
   malo → REJECTED; código bueno → CONNECTED como mano derecha (limb 1); 1439 muestras emitidas por el
   PC, todas con ambos flags, t_ns monótono, 0 paquetes perdidos, sincronización OK (rtt 0,1 ms), STATUS
   recibido (batería 0,6, 200 Hz) y BYE al final (`connected: false`).
3. **Capa Android compilada contra stubs**: compilé `MainActivity` y `MandoController` con kotlinc contra
   stubs Java escritos a mano de las clases `android.*` usadas (firmas de la API real). Sin errores ni
   avisos. No sustituye a compilar con el `android.jar` real.

## Qué no pude verificar y por qué

- `assembleDebug`/`testDebugUnitTest` con AGP: no hay Android SDK ni red (Maven/Google). Lo hará la CI.
- Comportamiento en un móvil real (tasa efectiva de sensores, base de `SensorEvent.timestamp`,
  recepción de broadcast con MulticastLock, latencia Wi-Fi). Pendiente de T3.

## Qué queda / ideas

- rtt mostrado = HELLO→WELCOME al emparejar; el rtt de sincronización lo calcula el PC (no se le envía
  al móvil). Podría mostrarse si el contrato añadiera ese dato al WELCOME o a un mensaje nuevo.
- Mostrar la extremidad cambiada desde el juego requiere que el PC reenvíe WELCOME (la app ya lo acepta
  en CONNECTED y actualiza la extremidad).
- Servicio en primer plano para seguir enviando con la pantalla apagada (hoy se exige pantalla encendida).

## Propuestas de contrato

- Ninguna obligatoria. Opcional: un mensaje PC→móvil con el rtt/offset estimado para mostrarlo en la app.
- El contrato no fija el periodo de STATUS (el simulador usa 1 s; la app 2 s, según el encargo). Sugerencia:
  "STATUS cada 1–2 s".
