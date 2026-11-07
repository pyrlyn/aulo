Fix: when a pipe instance connects, `GetNamedPipeClientProcessId` names the client process and that process's token user SID is compared with the daemon's. `LocalOwner` is granted only when they match and a second read still reports the same process id. A missing pid, an unreadable token, a different account, or a changed process id is refused with `authentication required`. `reject_remote_clients` and the pipe DACL stay as a second fence.

Check: `cargo test -p aulo-server auth` covers the SID comparison, the reused-pid refusal, and a request with no connect info. The Win32 probe and a health check over a real pipe run on the Windows CI runner.

The task as the old `done.md` described it:

`local_pipe` inserted `Caller::LocalOwner` for every named-pipe request. The default DACL also lets Administrators and SYSTEM write, which is weaker than the Unix `SO_PEERCRED` uid check and weaker than the module's fail-closed claim. Found by the 2026-10-07 audit.

The outcome as the old `plan.md` card recorded it:

Outcome: the accept path reads `GetNamedPipeClientProcessId` and the client process token, and `LocalOwner` is granted only when that account is the daemon's user. A missing identity, a different account, or a process id that changes during the read is refused with the same unauthenticated status as a Unix peer the kernel did not identify. The decision is tested on every host; the Win32 probe and a real pipe health check run on Windows.
