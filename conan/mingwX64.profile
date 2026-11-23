# Windows x86_64 as Kotlin/Native's mingw target links it. Conan Center has MSVC binaries only, so a library is
# built from the locked recipes with Kotlin/Native's clang and its mingw sysroot. The build passes their paths in
# (ConanInstallTask), with forward slashes: these values are Python literals, where a Windows `\U` is an escape.
{% set llvm = os.getenv("KONAN_LLVM") %}
{% set sysroot = os.getenv("KONAN_SYSROOT") %}
{% set windows = platform.system() == "Windows" %}
{% set exe = ".exe" if windows else "" %}
{% set target = ["--target=x86_64-w64-mingw32", "--sysroot=" + sysroot] %}
[settings]
os=Windows
arch=x86_64
# The ABI of the sysroot's GCC 9; the compiler itself is Kotlin/Native's clang.
compiler=gcc
compiler.version=9
compiler.libcxx=libstdc++11
compiler.threads=posix
compiler.exception=seh
build_type=Release

[conf]
tools.build:compiler_executables={"c": "{{ llvm }}/bin/clang{{ exe }}", "cpp": "{{ llvm }}/bin/clang++{{ exe }}"}
tools.build:cflags={{ target }}
# A C library's CMake project may enable C++ as well (nghttp2 does), and CMake then checks that compiler too.
tools.build:cxxflags={{ target }}
tools.build:sharedlinkflags={{ target + ["-fuse-ld=lld"] }}
tools.build:exelinkflags={{ target + ["-fuse-ld=lld"] }}
tools.build:sysroot={{ sysroot }}
tools.cmake.cmaketoolchain:system_name=Windows
tools.cmake.cmaketoolchain:system_processor=AMD64
{% if windows %}
# Windows has no make for CMake's default MinGW generator; Ninja comes from PATH, as CMake does.
tools.cmake.cmaketoolchain:generator=Ninja
{% else %}
# llvm-ar writes the symbol index itself; the host's ranlib would rewrite a COFF archive for its own format.
tools.cmake.cmaketoolchain:extra_variables={"CMAKE_RANLIB": "true"}
{% endif %}

[buildenv]
AR={{ llvm }}/bin/llvm-ar{{ exe }}
