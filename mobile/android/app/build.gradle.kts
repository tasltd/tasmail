import java.util.Properties
import java.io.FileInputStream

plugins {
    id("com.android.application")
    id("kotlin-android")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

// Added: TMAIL-56 — load release-signing credentials from android/key.properties.
// The file is gitignored; CI/operators copy key.properties.example, fill it in,
// and place the keystore at the configured storeFile path. If the file is
// missing we silently fall back to the debug signing config below so
// `flutter run --release` still works on a dev workstation.
val keystorePropertiesFile = rootProject.file("key.properties")
val keystoreProperties = Properties()
if (keystorePropertiesFile.exists()) {
    keystoreProperties.load(FileInputStream(keystorePropertiesFile))
}

android {
    namespace = "io.techatscale.tasmail_mobile"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = JavaVersion.VERSION_17.toString()
    }

    defaultConfig {
        applicationId = "io.techatscale.tasmail_mobile"
        minSdk = flutter.minSdkVersion
        targetSdk = flutter.targetSdkVersion
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    // Added: TMAIL-56 — production release signing config.
    // Activated only when android/key.properties is present. Allows the same
    // Gradle config to work for both unsigned dev builds (flutter run) and
    // signed production builds (fastlane / CI / release.sh).
    signingConfigs {
        create("release") {
            if (keystoreProperties.getProperty("storeFile") != null) {
                storeFile = file(keystoreProperties.getProperty("storeFile"))
                storePassword = keystoreProperties.getProperty("storePassword")
                keyAlias = keystoreProperties.getProperty("keyAlias")
                keyPassword = keystoreProperties.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            // Use production signing when key.properties is present, otherwise
            // fall back to debug signing so dev `flutter run --release` still
            // works without operators needing the production keystore.
            signingConfig = if (keystorePropertiesFile.exists()) {
                signingConfigs.getByName("release")
            } else {
                signingConfigs.getByName("debug")
            }

            // NOTE: minification stays OFF for now — TipTap / flutter_widget_from_html
            // pull in reflection-heavy code. Re-enable with a tested R8 ruleset once
            // we've measured the install-size win and validated no inbox/composer
            // runtime regressions.
            isMinifyEnabled = false
            isShrinkResources = false
        }
    }
}

flutter {
    source = "../.."
}
