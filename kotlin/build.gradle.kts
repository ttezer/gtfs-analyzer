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

// Maven Central (via its OSSRH-compatible staging endpoint) requires a POM with
// license/developer/scm info, plus sources and javadoc jars, before it will accept
// a publish — these aren't optional boilerplate, a publish without them is rejected.
java {
    withSourcesJar()
    withJavadocJar()
}

// sourcesJar bundles src/main/kotlin plus the generatedVersionDir source root added
// above; without this, Gradle only infers an implicit ordering (flagged as an
// unsafe task dependency) rather than a real one, and can run sourcesJar before
// Version.kt exists.
tasks.named("sourcesJar") { dependsOn(generateVersion) }

publishing {
    publications {
        create<MavenPublication>("maven") {
            from(components["java"])

            groupId = "io.github.transittrack"
            artifactId = "gtfs-analyzer"
            version = sdkVersion

            pom {
                name.set("gtfs-analyzer")
                description.set("JVM/Kotlin bindings for the GTFS Analyzer validation engine.")
                url.set("https://github.com/TransitTrack/gtfs-analyzer")
                licenses {
                    license {
                        name.set("MIT License")
                        url.set("https://opensource.org/licenses/MIT")
                    }
                }
                developers {
                    developer {
                        id.set("transittrack")
                        name.set("TransitTrack")
                        url.set("https://github.com/TransitTrack")
                    }
                }
                scm {
                    connection.set("scm:git:https://github.com/TransitTrack/gtfs-analyzer.git")
                    developerConnection.set("scm:git:ssh://git@github.com/TransitTrack/gtfs-analyzer.git")
                    url.set("https://github.com/TransitTrack/gtfs-analyzer")
                }
            }
        }
    }

    repositories {
        maven {
            name = "centralPortal"
            // Central Portal's legacy OSSRH-compatible ingestion endpoint — the
            // standard way to publish with plain maven-publish + signing, without
            // pulling in a third-party Gradle plugin.
            url = uri("https://ossrh-staging-api.central.sonatype.com/service/local/staging/deploy/maven2/")
            credentials {
                // Populated in CI via ORG_GRADLE_PROJECT_mavenCentralUsername /
                // ORG_GRADLE_PROJECT_mavenCentralPassword (see
                // .github/workflows/publish-kotlin.yml), which Gradle exposes here
                // as the project properties below.
                username = providers.gradleProperty("mavenCentralUsername").orNull
                password = providers.gradleProperty("mavenCentralPassword").orNull
            }
        }
    }
}

signing {
    // Populated in CI via ORG_GRADLE_PROJECT_signingInMemoryKey /
    // ORG_GRADLE_PROJECT_signingInMemoryKeyPassword (see
    // .github/workflows/publish-kotlin.yml). Locally, these are simply absent and
    // signing is skipped rather than failing the build.
    val signingKey = providers.gradleProperty("signingInMemoryKey").orNull
    val signingPassword = providers.gradleProperty("signingInMemoryKeyPassword").orNull
    if (signingKey != null && signingPassword != null) {
        useInMemoryPgpKeys(signingKey, signingPassword)
        sign(publishing.publications["maven"])
    }
}
