plugins {
    id("com.android.application")
    // AGP 9 supplies Kotlin itself, but NOT the Compose compiler — that stays a
    // separate plugin, and its version must track the Kotlin version AGP bundles
    // (2.2.10). A mismatch fails the build with a compiler-version error.
    id("org.jetbrains.kotlin.plugin.compose") version "2.2.10"
}

android {
    namespace = "net.protest.riot"
    compileSdk = 36

    defaultConfig {
        applicationId = "net.protest.riot"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        // Matches CFBundleShortVersionString on iOS/macOS. All three platforms
        // ship one version string; they are the same app.
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildFeatures {
        compose = true
    }

    sourceSets {
        getByName("main") {
            kotlin.directories.add(rootProject.file("../../build/generated/riot-ffi/uniffi").path)
            jniLibs.directories.add(rootProject.file("../../build/native/android/jniLibs").path)
            // Ships the built-in starter apps (checklist manifest+bundle) so the
            // directory can open them out of the box. Same packed .cbor files the
            // core embeds; referenced, never copied.
            assets.directories.add(rootProject.file("../../fixtures/apps").path)
        }
        getByName("androidTest") {
            assets.directories.add(rootProject.file("../../fixtures/apps").path)
        }
        getByName("test") {
            // WU-006B: the anchor protocol conformance vectors are read off the
            // host-JVM unit-test classpath (getResourceAsStream). Referenced from
            // the shared fixtures tree, never copied, so Rust/TS/Swift/Kotlin all
            // assert against the same bytes.
            resources.directories.add(rootProject.file("../../fixtures/anchor").path)
        }
    }
}

dependencies {
    implementation("net.java.dev.jna:jna:5.17.0@aar")
    implementation("androidx.webkit:webkit:1.14.0")

    // One BOM pins every Compose artifact to a tested-together set, so the
    // individual libraries below are deliberately version-less.
    val composeBom = platform("androidx.compose:compose-bom:2026.06.01")
    implementation(composeBom)
    androidTestImplementation(composeBom)
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.activity:activity-compose:1.12.0")
    debugImplementation("androidx.compose.ui:ui-tooling")

    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
    androidTestImplementation("junit:junit:4.13.2")
}
