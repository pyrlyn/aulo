# Spike T9.20: a separate OS user for GUI work

Question: can aulo run GUI work in a session the logged-in user cannot see or touch, the way Grok Bot and Muse use a VM and the way a Windows agent workspace uses another OS account?

Spec context: §6.3 (isolation levels). The three levels there are the user's desktop, a container (T13.5), and a separate OS user or VM (this spike).

## Summary

| Probe | Result on this host |
| --- | --- |
| Second Linux user, home mode `0700`, marker file mode `0600` | The calling user gets `PermissionError`. The file is not readable. |
| `Xvfb` as that user, display socket then mode `0700` | The calling user gets `PermissionError` on the socket. The isolated user connects. |
| `/dev/kvm` | Present, mode `0660`, group id 103. The unprivileged user is not in that group. |
| `qemu-system-x86_64`, Docker, Podman | Not installed. No VM was started. |
| macOS second user + Screen Sharing, Windows agent workspace | Not run. This host is Linux. |

Recommendation:

- A second local user plus a private X display is a real uid boundary on Linux. [`scripts/isolation-spike.py`](../../scripts/isolation-spike.py) is the prototype. It exits 0 only when both denials hold and the owner can connect, and it deletes the user afterwards.
- It is not a product session. There is no desktop, no screen sharing, and no input path. The lock is the home mode and the socket mode.
- A VM is a no-go on this host until QEMU (or a container runtime) is installed and the user can open `/dev/kvm`. Do not block the desktop default on it.
- macOS and Windows need their own probes. Nothing here says they work.
- The isolation to build next is the container in T13.5, not a host account created by the daemon.

## Setup

- Host: Linux `6.12.94+`, x86_64, user `ubuntu` with `sudo -n`. An X server was already on display `:1`. `Xvfb` is installed. `unshare` is installed. `bwrap` is not.
- The probe picks a free display in `:90`–`:109`. The run recorded here used `:90`.
- The temporary user is `aulo-spike`, shell `/usr/sbin/nologin`, home created by `useradd` and then mode `0700`.
- `Xvfb` is started as that user with `-nolisten tcp` so it does not open a TCP port. Access control inside X is disabled (`-ac`); the socket mode is what stops the other user. That is deliberate: the thing under test is the uid boundary, not MIT-MAGIC-COOKIE.
- After the checks, the probe kills every process of `aulo-spike`, deletes the user and the home, and removes the socket.

## Result

One run of `python3 scripts/isolation-spike.py` printed:

```text
file_denied=True
socket_denied=True
owner_connected=True
display=:90
```

Exit status 0. Afterwards `id aulo-spike` fails and `/tmp/.X11-unix` is back to the original `:1` socket only.

What this does not show:

- The logged-in user's X clients were not attacked, and the probe does not take over display `:1`.
- A compositor, a browser, or an input device was not placed in the isolated session.
- Screen sharing back to the desktop app was not built.
- `/dev/kvm` was not opened. A VM was not booted.
- Nothing was measured on macOS or Windows.

## Decision

Keep the user's desktop as the default (spec §6.3 level 1). Treat a separate OS account as a proven Linux uid boundary, not as a shipping session. Treat a VM as a no-go until a runtime and a usable `/dev/kvm` exist on the host that would run it. The container image (T13.5) is the isolation level to implement.
