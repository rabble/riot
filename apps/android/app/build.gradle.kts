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
        // Monotonic per release, supplied by scripts/android-release.sh from the
        // commit count. Play rejects a version code it has already seen, and 1
        // can only ever be uploaded once.
        versionCode = (findProperty("versionCode") as String?)?.toInt() ?: 1
        // Matches CFBundleShortVersionString on iOS/macOS. All three platforms
        // ship one version string; they are the same app.
        versionName = "0.1.1"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildFeatures {
        compose = true
    }

    // Release signing, for Play uploads.
    //
    // THE KEY IS NEVER IN THE REPO AND ITS PASSWORD IS NEVER ON DISK. The
    // keystore lives outside the checkout and the password comes from the macOS
    // keychain (service `riot-android-keystore`, account `riot-release-key`),
    // read by scripts/android-release.sh and passed in as a project property.
    // A checkout without those still builds — the release variant just comes out
    // unsigned rather than failing, so CI and contributors are unaffected.
    val keystorePath = (findProperty("riot.keystore") as String?)
        ?: System.getenv("RIOT_KEYSTORE_PATH")
    val keystorePassword = (findProperty("riot.keystorePassword") as String?)
        ?: System.getenv("RIOT_KEYSTORE_PASSWORD")
    val keystoreAlias = (findProperty("riot.keyAlias") as String?)
        ?: System.getenv("RIOT_KEY_ALIAS")
        ?: "riot"
    val canSign = keystorePath != null && keystorePassword != null && file(keystorePath).exists()

    signingConfigs {
        if (canSign) {
            create("release") {
                storeFile = file(keystorePath!!)
                storePassword = keystorePassword
                keyAlias = keystoreAlias
                // One password protects both the store and the key: keytool's
                // default when -keypass is not given separately.
                keyPassword = keystorePassword
            }
        }
    }

    buildTypes {
        getByName("release") {
            if (canSign) signingConfig = signingConfigs.getByName("release")
        }
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
