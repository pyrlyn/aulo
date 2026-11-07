Outcome: `scripts/isolation-spike.py` creates a temporary user and a private Xvfb, checks the other user is denied the home file and the display socket, and deletes the user. A VM is a no-go on this host (no QEMU or container runtime, `/dev/kvm` not group-accessible). Recorded in `docs/spikes/isolation.md`.

The outcome as the old `plan.md` card recorded it:

Outcome: `docs/spikes/isolation.md` and `scripts/isolation-spike.py`. On this Linux host a temporary user with a mode-0700 home and a private Xvfb cannot be read or connected to by the calling user, and the isolated user can open the display. The probe deletes the user. A VM was not started: QEMU, Docker and Podman are absent, and `/dev/kvm` is not usable by the unprivileged user. macOS and Windows were not probed. The desktop stays the default; the container in T13.5 is the isolation to build.
