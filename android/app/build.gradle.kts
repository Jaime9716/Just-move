import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.juegodebaile.mando"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.juegodebaile.mando"
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "0.1.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    lint {
        // El CI compila y pasa tests; un aviso de lint no debe romper assemble*.
        abortOnError = false
        checkReleaseBuilds = false
    }

    testOptions {
        unitTests {
            // La lógica pura no toca android.*; si algo lo tocara por error, que devuelva valores por
            // defecto en vez de lanzar "Method ... not mocked".
            isReturnDefaultValues = true
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

dependencies {
    // Sin dependencias de ejecución: solo APIs de la plataforma Android.
    testImplementation("junit:junit:4.13.2")
}
