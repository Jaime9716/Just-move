# App Android de mando (agente P)

Convierte un móvil Android en mando del Juego de Baile: envía acelerómetro + giroscopio a 200 Hz al PC
por UDP (puerto 47800) siguiendo `contracts/phone_protocol.md`. Compatible byte a byte con
`engine/jd_core/src/phone/protocol.rs` (lo comprueban los tests con vectores generados por el Rust real).

## Versiones (fijas y compatibles entre sí)

| Pieza | Versión | Notas |
|---|---|---|
| Gradle (wrapper) | 8.7 | mínimo que exige AGP 8.5.x; `gradle-wrapper.jar` oficial |
| Android Gradle Plugin | 8.5.2 | compileSdk/targetSdk 34, JDK 17 |
| Kotlin (KGP) | 2.0.21 | soporta AGP 7.1.3–8.5 y Gradle 6.8.3–8.8 |
| JDK | 17 | el mismo que usa la CI |
| minSdk | 26 | Android 8.0 |

Sin AndroidX, sin Compose y sin dependencias de ejecución: solo APIs de la plataforma
(`Activity`, Views, `SensorManager`, `DatagramSocket`). Única dependencia de test: JUnit 4.13.2.

## Compilar

Con Android SDK (CI o PC):

```sh
cd android
./gradlew assembleDebug testDebugUnitTest
# APK: app/build/outputs/apk/debug/app-debug.apk
```

Sin Android SDK ni red (entorno de desarrollo en la nube), solo la lógica pura y sus tests:

```sh
android/tools/test_core_offline.sh
```

(usa el compilador de Kotlin y JUnit que trae la instalación local de Gradle).

## Estructura

- `app/src/main/java/com/juegodebaile/mando/core/` — **lógica pura, sin `android.*`**:
  - `Protocol.kt`: codificar/decodificar los 8 mensajes (little-endian, cabecera 16 B, muestra 36 B).
  - `Sensors.kt`: `TimeBaseCorrector` (SensorEvent.timestamp → base `elapsedRealtimeNanos`, corrige si
    difiere > 1 s), `RateLimiter` (200 Hz), `SampleCombiner` (accel + último gyro si |Δt| ≤ 5 ms),
    `RateMeter`.
  - `PhoneClient.kt`: máquina de estados y temporizadores (DISCOVER cada 1 s → HELLO cada 500 ms →
    WELCOME aceptado/rechazado → envío; re-HELLO si 3 s sin oír al PC; lotes ≤ 8 muestras cada
    ~10 ms; SYNC_RESP inmediato; STATUS cada 2 s con batería, tasa real y perdidas; seq creciente).
  - `Endpoint.kt`: parseo de "IP:puerto" y del código de 4 dígitos.
- `MandoController.kt` — capa Android: sensores en hilo `jd-sensors` (samplingPeriodUs 5000,
  sin lotes en el hub), red en hilo `jd-net` (DatagramSocket con timeout 2 ms), MulticastLock y
  WifiLock de baja latencia, broadcast a 255.255.255.255 y al broadcast de cada interfaz.
- `MainActivity.kt` — UI por código: IP:puerto (se autorrellena con la respuesta a DISCOVER), código,
  Conectar/Desconectar, estado grande (Buscando PC… / Conectado como mano derecha / Código incorrecto /
  Sin conexión), tasa real y rtt. Pantalla siempre encendida y vertical. `device_id` aleatorio
  persistente en SharedPreferences.
- `app/src/test/resources/protocol_vectors.json` — vectores (campos + bytes hex) generados con
  `jd_core::phone::protocol::encode`.

## Uso

1. En el PC, arranca el juego (o `phone_sim`/`jd_record`): muestra IP:puerto y un código de 4 dígitos.
2. Móvil y PC en la misma Wi-Fi. Abre la app: si el PC responde al DISCOVER, la IP se rellena sola.
3. Escribe el código y pulsa **Conectar**. Sujeta el móvil en la mano derecha, pantalla hacia arriba.

La app envía mientras está en primer plano; al salir manda BYE y vuelve a conectar al regresar.

## Regenerar los vectores

Los vectores se generan con un programa temporal fuera del repo (crate con path-dep a
`engine/jd_core` que llama a `jd_core::phone::protocol::encode` y vuelca el JSON). Si cambia el
protocolo, repite ese paso; ver `docs/agents/P-report.md`.
