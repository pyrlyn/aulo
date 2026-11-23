# Linux x86_64 as Kotlin/Native links it: built from the locked recipes with Kotlin/Native's clang against its
# glibc 2.19 sysroot, so a library needs no newer glibc than the binary it is linked into.
{% set arch = "x86_64" %}
{% set processor = "x86_64" %}
{% set triple = "x86_64-unknown-linux-gnu" %}
{% include "linux.jinja" %}
