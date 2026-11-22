# Gradle tasks land with the first Kotlin/Native workspace task (plan S2).
default:
    @echo "Gradle workspace is not in the tree yet (plan S2). Until then: mise install, just api-docs."

# Regenerates docs/api from the protos. Needs network: the plugin runs on the BSR.
api-docs:
    buf generate proto
