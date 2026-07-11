plugins {
    id("com.android.application")
}

android {
    namespace = "org.riot.evidence"
    compileSdk = 36

    defaultConfig {
        applicationId = "org.riot.evidence"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1"
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
        }
    }
}

dependencies {
    implementation("net.java.dev.jna:jna:5.17.0@aar")

    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
    androidTestImplementation("junit:junit:4.13.2")
}
