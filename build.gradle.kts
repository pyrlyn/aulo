// What the plugins resolve to is locked as well (buildscript-gradle.lockfile).
buildscript {
    configurations.classpath {
        resolutionStrategy.activateDependencyLocking()
    }
}

plugins {
    id("aulo.graph")
}

// The catalog pins what the build asks for; the lock files (gradle.lockfile in every project)
// pin what it gets, transitive modules included, so a build does not change under an unchanged
// catalog. A lock entry is a strict constraint, so a changed catalog version takes no effect
// until the locks are rewritten. After changing a dependency:
// ./gradlew --write-locks resolveAndLockAll
// The root project resolves nothing, so Gradle writes no lock file for it; the committed empty
// gradle.lockfile is the one SonarCloud's text:S8569 looks for next to the root build file.
allprojects {
    dependencyLocking {
        lockAllConfigurations()
    }

    tasks.register("resolveAndLockAll") {
        description = "Resolves every configuration so that --write-locks records all of them."
        notCompatibleWithConfigurationCache("Resolves configurations at execution time")
        doFirst {
            require(gradle.startParameter.isWriteDependencyLocks) { "Run it with --write-locks" }
        }
        doLast {
            // The graph is what a lock records. Asking for the files as well fails where a
            // configuration leaves the choice of artifact to the task that consumes it.
            configurations.filter { it.isCanBeResolved }.forEach { it.incoming.resolutionResult.rootComponent.get() }
        }
    }
}
