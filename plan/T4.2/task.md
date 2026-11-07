# T4.2. Credentials in the OS keychain

Stage: S4 · Area: providers · Depends on: T4.1 · Blocks: 2 task(s)

aulo auth set/rm/ls <provider>: keys go to the keychain (keyring), env vars win, inline keys in config are rejected (the local model runner pattern). gRPC ConfigService can set a key but never return it.

Done when:

- test: config with an inline key fails with the file name
