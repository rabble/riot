plugins {
    id("com.android.application")
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

    lint {
        // UniFFI reflectively selects java.lang.ref.Cleaner only when present and otherwise uses pinned JNA.
        baseline = file("lint-baseline.xml")
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

    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
    androidTestImplementation("junit:junit:4.13.2")
}
