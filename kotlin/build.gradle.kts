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

// Local-dev convenience: `./gradlew test` (or `:compileTestKotlin`/an IDE run) builds and
// installs the current platform's native lib automatically, with no extra steps.
//
// This must NOT happen during CI's publish job. That job downloads per-platform native
// artifacts built by a matrix job (one leg per target, including linux-x86_64) and copies
// them into place with `copyNativeLib`, then runs `./gradlew publish`. `publish` also
// depends on `processResources`, so without an opt-out, CI would rerun a full LTO release
// `cargo build` for the runner's own platform (linux-x86_64, since the publish job runs on
// ubuntu-latest) right after copyNativeLib placed the matrix-built artifact there —
// overwriting it, making that matrix leg pointless, and risking a timeout from the rebuild.
//
// `-PskipNativeBuild=true` (passed by publish-kotlin.yml's publish job) skips both
// `dependsOn` wirings below, so `processResources`/`test` run with whatever native libs are
// already on disk (the ones copyNativeLib just placed) instead of triggering a rebuild.
val skipNativeBuild = providers.gradleProperty("skipNativeBuild").map { it.toBoolean() }.getOrElse(false)

if (!skipNativeBuild) {
    tasks.named("test") { dependsOn("buildNativeForCurrentPlatform") }

    // buildNativeForCurrentPlatform copies the native lib into src/main/resources/natives/<tag>/,
    // which processResources then copies into build/resources/main (what's actually on the test
    // classpath). `test dependsOn buildNativeForCurrentPlatform` alone only orders it before `test`
    // itself — processResources sits on a separate dependency branch (test -> classes ->
    // processResources) and Gradle was scheduling it before buildNativeForCurrentPlatform, so the
    // native lib resource missed this run's processResources copy. Order processResources after the
    // native build too so the freshly copied library is actually present when resources are processed.
    tasks.named("processResources") { dependsOn("buildNativeForCurrentPlatform") }
}

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
    // `providers.gradleProperty(...)` returns a `Provider<String>` that Gradle resolves
    // lazily at task-EXECUTION time, not during configuration/registration. The previous
    // `project.findProperty(...) as String? ?: error(...)` ran eagerly during registration,
    // so any command that must realize every registered task (`./gradlew tasks`, an IDE
    // Gradle sync) failed with this task's usage error even when nobody intended to run it.
    // Passing the providers straight to from()/into() keeps that validation lazy; if a
    // property is genuinely missing when the task actually runs, Gradle's file-collection
    // resolution calls Provider.get() internally and that throws a MissingValueException —
    // a perfectly good substitute for the old hand-rolled message, surfaced only when this
    // task actually executes rather than whenever Gradle merely registers it.
    val libFile = providers.gradleProperty("nativeLibFile")
    val tag = providers.gradleProperty("platformTag")
    from(libFile)
    into(tag.map { layout.projectDirectory.dir("src/main/resources/natives/$it") })
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
