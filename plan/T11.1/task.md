# T11.1. Plugin manifest

Stage: S11 · Area: plugins · Depends on: T2.5 · Blocks: 1 task(s)

aulo-plugin.toml with schemars schema: id, version, kinds (tools, stt, tts, llm-provider, skills, hooks), requested capabilities (net hosts, fs roots, exec, audio), entry (mcp-process, grpc-process, wasm).


Done when:

- schema test; invalid manifest names the file
