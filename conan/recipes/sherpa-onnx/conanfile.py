import os

from conan import ConanFile
from conan.errors import ConanInvalidConfiguration
from conan.tools.files import copy, download, get

required_conan_version = ">=2.0"


class SherpaOnnxConan(ConanFile):
    """sherpa-onnx's C API from the project's own prebuilt release archives.

    Conan Center and vcpkg have no sherpa-onnx, and its source build fetches onnxruntime and a dozen other
    libraries at configure time. The archives are checked against pinned SHA-256s (conandata.yml), so a
    "build" here is a verified download.

    Which archive each platform gets is set by what Kotlin/Native can link:
    - macOS: the static libraries (libc++, as Kotlin/Native links with).
    - Linux: the shared libraries. The static ones need a newer libstdc++ than Kotlin/Native's GCC 8
      sysroot has; the shared ones carry their C++ runtime and need only glibc 2.17 and GLIBCXX 3.4.19.
    - Windows: the DLLs and their import library. The static archives are MSVC C++ objects, which a
      mingw link cannot take.
    """

    name = "sherpa-onnx"
    version = "2.0.3"
    description = "Speech recognition, synthesis, VAD and keyword spotting on onnxruntime (prebuilt C API)"
    license = "Apache-2.0"
    url = "https://github.com/k2-fsa/sherpa-onnx"
    homepage = "https://github.com/k2-fsa/sherpa-onnx"
    settings = "os", "arch", "compiler", "build_type"

    _static = [
        "sherpa-onnx-c-api",
        "sherpa-onnx-core",
        "kaldi-decoder-core",
        "sherpa-onnx-kaldifst-core",
        "sherpa-onnx-fstfar",
        "sherpa-onnx-fst",
        "kaldi-native-fbank-core",
        "kaldi-native-io-core",
        "kaldi-hmm-gmm-core",
        "ssentencepiece_core",
        "kissfft-float",
        "utf8proc",
        "onnxruntime",
    ]

    @property
    def _key(self):
        return f"{self.settings.os}-{self.settings.arch}"

    def validate(self):
        if self._key not in self.conan_data["sources"][self.version]:
            raise ConanInvalidConfiguration(f"sherpa-onnx publishes no archive for {self._key}")

    def package_id(self):
        # The archive is the same whatever compiler or build type links it.
        del self.info.settings.compiler
        del self.info.settings.build_type

    def build(self):
        data = self.conan_data["sources"][self.version]
        get(self, **data[self._key], strip_root=True, destination="dist")
        download(self, filename="c-api.h", **data["header"])
        download(self, filename="LICENSE", **data["license"])

    def package(self):
        copy(self, "LICENSE", self.build_folder, os.path.join(self.package_folder, "licenses"))
        copy(self, "c-api.h", self.build_folder, os.path.join(self.package_folder, "include", "sherpa-onnx", "c-api"))
        lib = os.path.join(self.build_folder, "dist", "lib")
        out = os.path.join(self.package_folder, "lib")
        if self.settings.os == "Windows":
            copy(self, "sherpa-onnx-c-api.lib", lib, out)
            for dll in ("sherpa-onnx-c-api.dll", "onnxruntime.dll", "onnxruntime_providers_shared.dll"):
                copy(self, dll, lib, os.path.join(self.package_folder, "bin"))
        elif self.settings.os == "Linux":
            for so in ("libsherpa-onnx-c-api.so", "libonnxruntime.so"):
                copy(self, so, lib, out)
        else:
            for name in self._static:
                copy(self, f"lib{name}.a", lib, out)

    def package_info(self):
        if self.settings.os == "Macos":
            self.cpp_info.libs = list(self._static)
            self.cpp_info.system_libs = ["c++"]
            self.cpp_info.frameworks = ["Foundation"]
        else:
            self.cpp_info.libs = ["sherpa-onnx-c-api"]
