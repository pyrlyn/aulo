# The machine Conan runs on. Nothing is built for it: every package here is a library for a target.
[settings]
os={{ detect_api.detect_os() }}
arch={{ detect_api.detect_arch() }}
