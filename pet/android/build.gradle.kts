plugins {
    id("com.android.application") version "8.13.2"
    id("org.jetbrains.kotlin.android") version "2.3.20"
    id("org.jetbrains.kotlin.plugin.compose") version "2.3.20"
}

val petAssets by tasks.registering(Sync::class) {
    from("../ios/Resources") { include("pet-native.js", "demo.jsonl") }
    from("../whale-points.tsv")
    from("../LICENSE")
    from("../NOTICE")
    from("THIRD_PARTY_NOTICES.txt")
    into(layout.buildDirectory.dir("generated/petAssets"))
}
val portableSources by tasks.registering(Sync::class) {
    from(files("PetSim.kt", "CodewhalePet.kt"))
    into(layout.buildDirectory.dir("generated/portableKotlin"))
}

android {
    namespace = "codewhale.pet"
    compileSdk = 35
    defaultConfig {
        applicationId = "dev.shannonlabs.codewhale.pet"
        minSdk = 26
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }
    sourceSets["main"].apply {
        java.setSrcDirs(listOf("src/main/kotlin"))
        java.srcDir(layout.buildDirectory.dir("generated/portableKotlin"))
        assets.srcDir(layout.buildDirectory.dir("generated/petAssets"))
    }
    buildFeatures { compose = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    packaging { resources.excludes += "/META-INF/{AL2.0,LGPL2.1}" }
    lint { abortOnError = true }
}
tasks.named("preBuild") { dependsOn(petAssets, portableSources) }
kotlin { jvmToolchain(17) }

dependencies {
    implementation(platform("androidx.compose:compose-bom:2025.04.01"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.9.0")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.9.0")
    implementation("app.cash.zipline:zipline:1.27.0")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test:runner:1.6.2")
    androidTestImplementation(platform("androidx.compose:compose-bom:2025.04.01"))
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}
