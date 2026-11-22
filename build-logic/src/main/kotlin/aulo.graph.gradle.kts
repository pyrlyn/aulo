import aulo.buildlogic.CheckModuleGraphTask

// Applied by the root project only: checks the module-role rules of T2.2 across all modules.
evaluationDependsOnChildren()

val checkModuleGraph =
    tasks.register<CheckModuleGraphTask>("checkModuleGraph") {
        group = "verification"
        description = "Fails when a module breaks the module-role rules (contracts, surfaces, heavy dependencies)."
        subprojects.forEach { manifests.from(it.tasks.named("moduleManifest")) }
        report.set(layout.buildDirectory.file("aulo/module-graph.txt"))
    }

tasks.register("test") {
    group = "verification"
    description = "Checks the module graph and runs the build-logic tests."
    dependsOn(checkModuleGraph, gradle.includedBuild("build-logic").task(":test"))
}
