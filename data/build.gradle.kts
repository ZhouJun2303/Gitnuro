plugins {    // Apply the shared build logic from a convention plugin.
    // The shared code is located in `buildSrc/src/main/kotlin/kotlin-jvm.gradle.kts`.
    id("buildsrc.convention.kotlin-jvm")

    alias(libs.plugins.ksp)
    alias(libs.plugins.kotlinx.serialization)
}

dependencies {
    implementation(project(":common"))
    implementation(project(":domain"))

    implementation(libs.jgit.core)
    implementation(libs.jgit.gpg)
    implementation(libs.jgit.lfs)

    implementation(libs.coroutines)

    implementation(libs.datastore)
    implementation(libs.datastore.preferences)

    implementation(libs.dagger)
    ksp(libs.dagger.compiler)

    implementation(libs.kotlinx.serialization.json)

    testImplementation(platform(libs.junit.bom))
    testImplementation(libs.junit.jupiter)
    testRuntimeOnly(libs.junit.platform.launcher)

    testImplementation(libs.mockk)
    testImplementation(libs.ktor.client.mock)

    implementation(libs.kotlin.logging)
    implementation(libs.slf4j.api)
    implementation(libs.slf4j.reload4j)

    implementation(libs.bouncycastle)

    implementation(libs.ktor.client)
    implementation(libs.ktor.client.cio)
    implementation(libs.ktor.client.content.negotiation)
    implementation(libs.ktor.serialization.kotlinx.json)
    implementation(libs.ktor.client.logging)
}
