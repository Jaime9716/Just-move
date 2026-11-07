// App Android de mando del Juego de Baile (agente P).
//
// Versiones fijas y compatibles entre sí (tabla oficial de compatibilidad):
//   Gradle 8.7 (wrapper)                 -> mínimo exigido por AGP 8.5.x
//   Android Gradle Plugin 8.5.2          -> compileSdk/targetSdk 34, build-tools 34.0.0, JDK 17
//   Kotlin (KGP) 2.0.21                  -> soporta AGP 7.1.3 .. 8.5 y Gradle 6.8.3 .. 8.8
// Sin AndroidX ni Compose: la app solo usa APIs de la plataforma.
plugins {
    id("com.android.application") version "8.5.2" apply false
    id("org.jetbrains.kotlin.android") version "2.0.21" apply false
}
