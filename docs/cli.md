# The `aulo` and `aulod` command lines

aulo is in early development, so the CLI covers configuration, provider keys and the speech
benchmark; chats go through the desktop apps. The commands marked *run* below were executed on a
build of `main` (`cargo build -p aulo`, binaries `target/debug/aulo` and `aulod`) with
`AULO_HOME` pointing at an empty scratch directory; the ones marked *help only* were checked
against `--help` and not started.

```bash
export AULO_HOME="$(mktemp -d)"    # where config.toml lives (default: ~/.aulo)
```

## Configuration *(run)*

Print the default configuration file, every key commented out with an explanation:

```bash
aulo config default > $AULO_HOME/config.toml
```

Settings are layered, lowest first: the built-in defaults, `$AULO_HOME/config.toml`, the project's
`.aulo.toml`, `AULO_*` environment variables (`__` separates levels), and command-line flags. Show
the effective result, secrets masked, and where each value came from:

```bash
aulo config show              # the effective configuration
aulo config show --origin     # the same, each line tagged with its source
```

```
plugins.dirs = []  # built-in defaults
policy.approval_timeout_secs = 60  # built-in defaults
...
```

Override one key for a single command with the environment:

```bash
AULO_VOICE__LISTEN=wake-word aulo config show --origin | grep listen
```

```
voice.listen = "wake-word"  # AULO_VOICE__LISTEN
```

`voice.listen` takes `push-to-talk` or `wake-word`; any other value is refused with the
variable named in the error.

## Provider keys *(run `ls`; help only for `set` and `rm`)*

API keys never go in the config file; an `api_key` setting is rejected. They live in the OS
keychain, and the environment wins over it (`AULO_OPENAI_API_KEY`, then `OPENAI_API_KEY`).

```bash
aulo auth ls                         # providers and whether a key is available
printf '%s\n' "$KEY" | aulo auth set openai   # the key is one line on stdin
aulo auth rm openai
```

On a fresh home, `aulo auth ls` prints `no providers configured`. Values are never printed.
Set `AULO_KEYCHAIN=off` to disable the keychain.

## The daemon *(help only)*

```bash
aulod                              # run in the foreground (same as `aulod run`)
aulod --listen 127.0.0.1:7878      # also accept remote clients (TLS required, see config)
aulod service install              # start at login as a user service
aulod service status               # prints "not installed" when there is none
aulod service uninstall
```

`aulod service status` was run and printed `not installed` on a machine without the service.
With `--listen` (or `daemon.listen`), the config needs `tls_cert` and `tls_key`.

## Speech benchmark *(help only)*

```bash
aulo bench speech      # word error rate, real-time factor, time to first audio per engine
```
