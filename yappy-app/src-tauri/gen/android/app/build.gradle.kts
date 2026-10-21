import java.util.Properties

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

android {
    compileSdk = 36
    namespace = "com.yappy.app"
    // La voz pregrabada del paseo se abre con AssetManager.openFd: los .m4a
    // tienen que ir SIN comprimir dentro del APK/AAB.
    androidResources {
        noCompress += listOf("m4a")
        // Tauri deja los resources/ en assets/_up_/resources, y el patrón por
        // defecto de aapt descarta las carpetas que empiezan por «_»
        // (`<dir>_*`): el APK salía sin cuentos, muestras ni voz del paseo.
        // Es el patrón por defecto de AGP sin esa entrada, más lo que el móvil
        // no usa y Tauri no borra de assets/ si quedó de un build anterior
        // (lo mismo que project.yml quita en iOS: pdfium, extension, paddleocr; y
        // el yappy-ocr.swift del Mac, que cae en _up_/macos).
        ignoreAssetsPattern = "!.svn:!.git:!.ds_store:!*.scc:.*:!CVS:!thumbs.db:!picasa.ini:!*~" +
            ":<dir>pdfium:<dir>extension:<dir>paddleocr:<dir>macos"
    }
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "com.joseluissaorin.yappy.android"
        minSdk = 24
        targetSdk = 36
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
                jniLibs.keepDebugSymbols.add("*/armeabi-v7a/*.so")
                jniLibs.keepDebugSymbols.add("*/x86/*.so")
                jniLibs.keepDebugSymbols.add("*/x86_64/*.so")
            }
        }
        getByName("release") {
            isMinifyEnabled = true
            proguardFiles(
                *fileTree(".") { include("**/*.pro") }
                    .plus(getDefaultProguardFile("proguard-android-optimize.txt"))
                    .toList().toTypedArray()
            )
        }
    }
    kotlinOptions {
        jvmTarget = "1.8"
    }
    buildFeatures {
        buildConfig = true
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    // La sesión de medios (pantalla de bloqueo, auriculares) y la notificación MediaStyle.
    implementation("androidx.media:media:1.7.0")
    // Yappy Parlanchín: RevenueCat sobre Google Play Billing.
    implementation("com.revenuecat.purchases:purchases:10.22.1")
    // La reseña: el aviso nativo de valoración de Google Play.
    implementation("com.google.android.play:review:2.0.2")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = "tauri.build.gradle.kts")