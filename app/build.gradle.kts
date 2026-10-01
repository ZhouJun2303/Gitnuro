import org.gradle.jvm.tasks.Jar
import org.jetbrains.compose.desktop.application.dsl.TargetFormat

plugins {    // Apply the shared build logic from a convention plugin.
    // The shared code is located in `buildSrc/src/main/kotlin/kotlin-jvm.gradle.kts`.
    id("buildsrc.convention.kotlin-jvm")

    // Apply the Application plugin to add support for building an executable JVM application.
    //application
    alias(libs.plugins.compose.compiler)
    alias(libs.plugins.compose)
    alias(libs.plugins.ksp)
    alias(libs.plugins.kotlinx.serialization)
}

val javaLanguageVersion = JavaLanguageVersion.of(25)
val linuxArmTarget = "aarch64-unknown-linux-gnu"
val linuxX64Target = "x86_64-unknown-linux-gnu"

// Remember to update Constants.APP_VERSION when changing this version
val projectVersion = "2.0-beta03"

val projectName = "AweGit"

// Required for JPackage, as it doesn't accept additional suffixes after the version.
val projectVersionSimplified = "2.0.0"

val rustGeneratedSource = "${layout.buildDirectory.get()}/generated/source/uniffi/main/com/zhoujun/awegit/java"

val packageName = "com.zhoujun.awegit"
group = packageName
version = projectVersion

val isLinuxAarch64 = (properties.getOrDefault("isLinuxAarch64", "false") as String).toBoolean()
val useCross = (properties.getOrDefault("useCross", "false") as String).toBoolean()
val isRustRelease = (properties.getOrDefault("isRustRelease", "true") as String).toBoolean()


sourceSets.getByName("main") {
    kotlin.srcDir(rustGeneratedSource)
}

sourceSets.main.get().java.srcDirs("app/src/main/resources").includes.addAll(arrayOf("**/*.*"))

dependencies {
    implementation(project(":common"))
    implementation(project(":data"))
    implementation(project(":domain"))

    val composeDependency = when {
        currentOs() == OS.LINUX && isLinuxAarch64 -> libs.compose.desktop.linux.arm64
        else -> compose.desktop.currentOs
    }

    println("composeDependency: $composeDependency")
    implementation(composeDependency)

    implementation(libs.compose.ui.util)
    implementation(libs.compose.components.animatedimage)
    implementation(libs.compose.components.resources)
    implementation(libs.compose.material.icons.extended)
    implementation(libs.compose.navigation3)

    implementation(libs.jgit.core)
    implementation(libs.jgit.gpg)
    implementation(libs.jgit.lfs)

    implementation(libs.coroutines)
    implementation(libs.kotlinx.coroutines.jvm)

    implementation(libs.kotlinx.serialization.json)
    implementation(libs.dagger)
    ksp(libs.dagger.compiler)

    testImplementation(platform(libs.junit.bom))
    testImplementation(libs.junit.jupiter)
    testRuntimeOnly(libs.junit.platform.launcher)

    testImplementation(libs.mockk)

    implementation(libs.kotlin.logging)
    implementation(libs.slf4j.api)
    implementation(libs.slf4j.reload4j)

    implementation(libs.bouncycastle)

    implementation(libs.ktor.client)
    implementation(libs.ktor.client.cio)
    implementation(libs.ktor.client.content.negotiation)
    implementation(libs.ktor.serialization.kotlinx.json)
    implementation(libs.ktor.client.logging)

    implementation(libs.coil3.compose)
    implementation(libs.coil3.network.okhttp)

    implementation(libs.datastore)
    implementation(libs.datastore.preferences)

    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(libs.androidx.lifecycle.viewmodel.navigation3)
}

fun currentOs(): OS {
    val os = System.getProperty("os.name")
    return when {
        os.equals("Mac OS X", ignoreCase = true) -> OS.MAC
        os.startsWith("Win", ignoreCase = true) -> OS.WINDOWS
        os.startsWith("Linux", ignoreCase = true) -> OS.LINUX
        else -> error("Unknown OS name: $os")
    }
}

enum class OS {
    LINUX,
    WINDOWS,
    MAC
}

tasks.test {
    useJUnitPlatform()
    testLogging {
        events("passed", "skipped", "failed")
    }
}

kotlin {
    jvmToolchain {
        languageVersion.set(javaLanguageVersion)
    }
}

composeCompiler {
    stabilityConfigurationFiles.add(project.layout.projectDirectory.file("compose-stability.conf"))
}

tasks.named("compileKotlin", org.jetbrains.kotlin.gradle.tasks.KotlinCompilationTask::class.java) {
    compilerOptions {
        allWarningsAsErrors.set(false)
        freeCompilerArgs.addAll("-opt-in=kotlin.RequiresOptIn")
    }
}

compose.desktop {
    application {
        mainClass = "com.zhoujun.awegit.MainKt"

        sourceSets.forEach {
            it.java.srcDir(rustGeneratedSource)
        }

        nativeDistributions {
            includeAllModules = true
            packageName = projectName
            version = projectVersionSimplified
            description = "Multiplatform Git client"
            targetFormats(TargetFormat.Dmg)

            windows {
                iconFile.set(project.file("../icons/icon.ico"))
            }

            macOS {
                jvmArgs(
                    "-Dapple.awt.application.appearance=system"
                )
                iconFile.set(project.file("../icons/icon.icns"))
                bundleID = packageName
                signing {
                    sign.set(System.getenv("SIGNING_IDENTITY") != null)
                    identity.set(providers.environmentVariable("SIGNING_IDENTITY"))
                }
                notarization {
                    val providers = project.providers
                    appleID.set(providers.environmentVariable("NOTARIZATION_APPLE_ID"))
                    password.set(providers.environmentVariable("NOTARIZATION_PASSWORD"))
                    teamID.set(providers.environmentVariable("NOTARIZATION_TEAM_ID"))
                }
            }
        }
    }
}

val appImageDir = layout.buildDirectory.dir("compose/binaries/main/app")
val appInputDir = layout.buildDirectory.dir("compose/app-input")

val stageAppInput = tasks.register<Copy>("stageAppInput") {
    dependsOn(tasks.named("jar"))
    from(tasks.named<Jar>("jar").map { it.archiveFile })
    from(configurations.named("runtimeClasspath")) {
        eachFile {
            val source = file ?: return@eachFile
            val group = source.parentFile?.parentFile?.parentFile?.parentFile?.name ?: "lib"
            name = "${group}__${source.name}"
        }
    }
    into(appInputDir)
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE
}

tasks.register<Exec>("testExe") {
    group = "distribution"
    description = "Build AweGit.exe with the current JDK. This JDK has no jmods, so jlink cannot create a runtime image."
    dependsOn(stageAppInput)
    val destDir = appImageDir.get().asFile
    val inputDir = appInputDir.get().asFile
    val javaHome = File(System.getProperty("java.home"))
    val jarName = tasks.named<Jar>("jar").get().archiveFileName.get()
    doFirst {
        destDir.resolve(projectName).deleteRecursively()
        destDir.mkdirs()
    }
    commandLine(
        File(javaHome, "bin/jpackage.exe").absolutePath,
        "--type", "app-image",
        "--name", projectName,
        "--dest", destDir.absolutePath,
        "--input", inputDir.absolutePath,
        "--main-jar", jarName,
        "--main-class", "com.zhoujun.awegit.MainKt",
        "--runtime-image", javaHome.absolutePath,
        "--icon", rootProject.file("icons/icon.ico").absolutePath,
    )
    doLast {
        val outExe = destDir.resolve(projectName).resolve("$projectName.exe")
        val csc = File(System.getenv("WINDIR") ?: "C:\\Windows", "Microsoft.NET\\Framework64\\v4.0.30319\\csc.exe")
        val code = ProcessBuilder(
            csc.absolutePath,
            "/nologo",
            "/target:winexe",
            "/win32icon:${rootProject.file("icons/icon.ico").absolutePath}",
            "/out:${outExe.absolutePath}",
            rootProject.file("app/packaging/AweGitLauncher.cs").absolutePath,
        ).inheritIO().start().waitFor()
        if (code != 0) {
            error("Failed to compile AweGit.exe launcher, exit code $code")
        }
    }
}


tasks.register("fatJarLinux", type = Jar::class) {
    val archSuffix = if (isLinuxAarch64) {
        "arm_aarch64"
    } else {
        "x86_64"
    }

    archiveBaseName.set("$projectName-linux-$archSuffix-$projectVersion")
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE

    manifest {
        attributes["Implementation-Title"] = name
        attributes["Implementation-Version"] = projectVersion
        attributes["Main-Class"] = "com.zhoujun.awegit.MainKt"
    }
    from(configurations.runtimeClasspath.get().map { if (it.isDirectory) it else zipTree(it) }) {
        exclude(
            "META-INF/MANIFEST.MF",
            "META-INF/*.SF",
            "META-INF/*.DSA",
            "META-INF/*.RSA",
        )
    }
    with(tasks.jar.get() as CopySpec)
}

tasks.named("compileKotlin") {
    dependsOn("rustTasks")
}

tasks.named("processResources") {
    dependsOn("rustTasks")
}

tasks.register("tasksList") {
    println("Tasks")
    tasks.forEach {
        println("- ${it.name}")
    }
}

val rustProjectDir = File(project.projectDir.parent, "rs")
val rustTargetDir = rustProjectDir.resolve("target")
val generatedKotlinDir = rootProject.layout.projectDirectory.dir(
    "domain/src/main/kotlin/com/zhoujun/awegit/autogenerated"
)
val libName = when (currentOs()) {
    OS.LINUX -> "libawegit_rs.so"
    OS.WINDOWS -> "awegit_rs.dll"
    OS.MAC -> "libawegit_rs.dylib"
}
val cargoInputs = listOf(
    rustProjectDir.resolve("Cargo.toml"),
    rustProjectDir.resolve("Cargo.lock"),
    rustProjectDir.resolve("uniffi.toml"),
    rustProjectDir.resolve("uniffi-bindgen.rs"),
    rustProjectDir.resolve("rust-toolchain.toml"),
)

fun cargoExecutable(): String = findBinaryInPath("cargo") ?: "cargo"

fun rustProfileDir(release: Boolean): File {
    val profile = if (release) "release" else "debug"
    if (currentOs() == OS.LINUX && useCross && release) {
        val triple = if (isLinuxAarch64) linuxArmTarget else linuxX64Target
        return rustTargetDir.resolve(triple).resolve(profile)
    }
    return rustTargetDir.resolve(profile)
}

val cargoBuildDebug = tasks.register<Exec>("cargoBuildDebug") {
    workingDir = rustProjectDir
    commandLine(cargoExecutable(), "build")
    environment("CARGO_TARGET_DIR", rustTargetDir.absolutePath)
    inputs.dir(rustProjectDir.resolve("src"))
    inputs.files(cargoInputs)
    outputs.file(rustProfileDir(false).resolve(libName))
}

if (isRustRelease) {
    tasks.register<Exec>("cargoBuildRelease") {
        workingDir = rustProjectDir
        val triple = if (isLinuxAarch64) linuxArmTarget else linuxX64Target
        val command = mutableListOf(cargoExecutable(), "build", "--release")
        if (currentOs() == OS.LINUX && useCross) {
            command.add("--target=$triple")
        }
        setCommandLine(command)
        environment("CARGO_TARGET_DIR", rustTargetDir.absolutePath)
        inputs.dir(rustProjectDir.resolve("src"))
        inputs.files(cargoInputs)
        inputs.property("useCross", useCross)
        inputs.property("isLinuxAarch64", isLinuxAarch64)
        outputs.file(rustProfileDir(true).resolve(libName))
    }
}

tasks.register<Exec>("generateUniffiKotlin") {
    dependsOn(cargoBuildDebug)
    workingDir = rustProjectDir
    val outDir = generatedKotlinDir.asFile
    commandLine(
        cargoExecutable(),
        "run",
        "--bin",
        "uniffi-bindgen",
        "generate",
        "--language",
        "kotlin",
        "--out-dir",
        outDir.absolutePath,
        "--library",
        rustProfileDir(false).resolve(libName).absolutePath,
        "--config",
        "uniffi.toml",
    )
    environment("CARGO_TARGET_DIR", rustTargetDir.absolutePath)
    inputs.file(rustProfileDir(false).resolve(libName))
    inputs.file(rustProjectDir.resolve("uniffi.toml"))
    outputs.dir(generatedKotlinDir)
    doFirst {
        if (outDir.exists()) {
            outDir.listFiles()?.forEach { file -> if (file.name != ".gitignore") file.deleteRecursively() }
        } else {
            outDir.mkdirs()
        }
    }
}

tasks.register<Copy>("copyRustLib") {
    val release = isRustRelease
    dependsOn(if (release) "cargoBuildRelease" else cargoBuildDebug)
    from(rustProfileDir(release).resolve(libName))
    into(layout.projectDirectory.dir("src/main/resources"))
}

tasks.register("rustTasks") {
    dependsOn(cargoBuildDebug, "generateUniffiKotlin", "copyRustLib")
    if (isRustRelease) {
        dependsOn("cargoBuildRelease")
    }
}

tasks.register("rust_copyBuild") {
    dependsOn("copyRustLib")
}

gradle.projectsEvaluated {
    val rust = tasks.named("rustTasks")
    val domainProject = rootProject.project(":domain")
    listOf("kspKotlin", "compileKotlin").forEach { taskName ->
        domainProject.tasks.named(taskName).configure {
            dependsOn(rust)
        }
    }
}

fun findBinaryInPath(binaryName: String): String? {
    val path = System.getenv("PATH") ?: return null
    val names = if (currentOs() == OS.WINDOWS) listOf("$binaryName.exe", binaryName) else listOf(binaryName)
    for (directory in path.split(File.pathSeparator)) {
        for (name in names) {
            val candidate = File(directory, name)
            if (candidate.isFile && candidate.canExecute()) {
                return candidate.absolutePath
            }
        }
    }
    return null
}



