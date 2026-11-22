# Host-target tests and style, the same checks CI runs.
default: test lint

test:
    ./gradlew test

lint:
    ./gradlew ktlintCheck

# Links the `aulo` and `aulod` executables for every target this host can build.
build:
    ./gradlew assemble

# Regenerates docs/api from the protos. Needs network: the plugin runs on the BSR.
api-docs:
    buf generate proto
