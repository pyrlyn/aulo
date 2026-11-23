# aulo documentation

aulo is in early development; user-facing guides arrive with the first release.

| Page | What |
| --- | --- |
| [cli.md](cli.md) | The `aulo` and `aulod` command lines: config layers, provider keys, the service |
| [api/aulo.v1.md](api/aulo.v1.md) | gRPC API reference for `aulod`, generated from `proto/` (`just api-docs`) |
| [spikes/aec.md](spikes/aec.md) | Spike report: acoustic echo cancellation |
| [spikes/sherpa-onnx.md](spikes/sherpa-onnx.md) | Spike report: sherpa-onnx speech engines |
| [threat-model.md](threat-model.md) | Trust boundaries, STRIDE, and the guard for each threat |
| [design.md](design.md) | Where the Figma design lives, the desktop and mobile designs, safety in the UI |

The spike reports were written against the retired Rust workspace; they stay as
research and must be re-verified on Kotlin/Native (see T0.6 and the other spike tasks).

Planned here: the user guide (T15.4) and the API guide (T15.5) — see
[plan/](../plan/README.md). The full specification is [spec.md](../spec.md).
