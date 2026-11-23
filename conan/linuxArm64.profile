# Linux aarch64 as Kotlin/Native links it: built from the locked recipes with Kotlin/Native's clang against its
# glibc 2.25 sysroot. Conan Center has no Linux aarch64 binaries.
{% set arch = "armv8" %}
{% set processor = "aarch64" %}
{% set triple = "aarch64-unknown-linux-gnu" %}
{% include "linux.jinja" %}
