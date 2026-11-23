# macOS arm64. Matches Conan Center's prebuilt binaries, and the build installs with `--build=never`, so
# nothing is compiled: Kotlin/Native links macOS with the host's Xcode, which a profile cannot pin.
[settings]
os=Macos
arch=armv8
compiler=apple-clang
compiler.version=17
compiler.libcxx=libc++
build_type=Release
