# T4.4. Model catalog and per-chat model switching

Stage: S4 · Area: providers · Depends on: T4.1, T1.5, T3.5 · Blocks: 3 task(s)

ConfigService lists providers and models (catalog plus served overlay). A chat stores its provider/model; SwitchModel works mid-chat.

Done when:

- test switches model between turns and the next request uses it
