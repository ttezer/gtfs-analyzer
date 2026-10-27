import java.io.File

plugins {
    kotlin("jvm") version "2.0.21"
    kotlin("plugin.serialization") version "2.0.21"
    `maven-publish`
    signing
}

group = "io.github.transittrack"

val cargoToml = rootProject.projectDir.resolve("../Cargo.toml").readText()
val sdkVersion = Regex("""\[workspace\.package][^\[]*?version\s*=\s*"([^"]+)"""", RegexOption.DOT_MATCHES_ALL)
    .find(cargoToml)
    ?.groupValues
    ?.get(1)
    ?: error("could not find [workspace.package] version in ../Cargo.toml")

version = sdkVersion

kotlin {
    jvmToolchain(17)
}

repositories {
    mavenCentral()
}

dependencies {
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")
    testImplementation(kotlin("test"))
}

tasks.test {
    useJUnitPlatform()
}

tasks.named("test") { dependsOn("buildNativeForCurrentPlatform") }

// buildNativeForCurrentPlatform copies the native lib into src/main/resources/natives/<tag>/,
// which processResources then copies into build/resources/main (what's actually on the test
// classpath). `test dependsOn buildNativeForCurrentPlatform` alone only orders it before `test`
// itself — processResources sits on a separate dependency branch (test -> classes ->
// processResources) and Gradle was scheduling it before buildNativeForCurrentPlatform, so the
// native lib resource missed this run's processResources copy. Order processResources after the
// native build too so the freshly copied library is actually present when resources are processed.
tasks.named("processResources") { dependsOn("buildNativeForCurrentPlatform") }

val generatedVersionDir = layout.buildDirectory.dir("generated/version")

val generateVersion = tasks.register("generateVersion") {
    val outputDir = generatedVersionDir
    inputs.property("sdkVersion", sdkVersion)
    outputs.dir(outputDir)

    doLast {
        val packageDir = outputDir.get().dir("io/github/transittrack/gtfsanalyzer").asFile
        packageDir.mkdirs()
        File(packageDir, "Version.kt").writeText(
            """
            package io.github.transittrack.gtfsanalyzer

            const val SDK_VERSION: String = "$sdkVersion"

            """.trimIndent()
        )
    }
}

kotlin {
    sourceSets {
        main {
            kotlin.srcDir(generatedVersionDir)
        }
    }
}

tasks.named("compileKotlin") {
    dependsOn(generateVersion)
}

tasks.register<Copy>("copyNativeLib") {
    val libFile = (project.findProperty("nativeLibFile") as String?)
        ?: error("usage: -PnativeLibFile=<path> -PplatformTag=<tag>")
    val tag = (project.findProperty("platformTag") as String?)
        ?: error("usage: -PnativeLibFile=<path> -PplatformTag=<tag>")
    from(libFile)
    into(layout.projectDirectory.dir("src/main/resources/natives/$tag"))
}

tasks.register("buildNativeForCurrentPlatform") {
    doLast {
        val repoRoot = rootDir.parentFile // kotlin/ is a direct child of the repo root
        exec { workingDir = repoRoot; commandLine("cargo", "build", "--release", "-p", "gtfs-analyzer-jvm") }
        val os = org.gradle.internal.os.OperatingSystem.current()
        val arch = System.getProperty("os.arch")
        val tag = when {
            os.isMacOsX && arch.contains("aarch64") -> "darwin-aarch64"
            os.isLinux && arch.contains("aarch64") -> "linux-aarch64"
            os.isLinux -> "linux-x86_64"
            else -> error("unsupported host platform for local native build: $os/$arch")
        }
        val ext = if (os.isMacOsX) "dylib" else "so"
        val built = repoRoot.resolve("target/release/libgtfs_jvm.$ext")
        copy {
            from(built)
            into(layout.projectDirectory.dir("src/main/resources/natives/$tag"))
        }
    }
}
