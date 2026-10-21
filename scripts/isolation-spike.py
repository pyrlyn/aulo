#!/usr/bin/env python3
"""Probe a separate OS user with a private X display.

This is the T9.20 prototype. It creates a temporary user, a mode-0700 home,
and an Xvfb the other user cannot open. It deletes the user on the way out.
Re-run as root, or as a user who can `sudo -n`.

Exit 0 when the other user is denied both the home file and the display
socket, and the isolated user can open the socket.
"""

from __future__ import annotations

import os
import socket
import subprocess
import sys
import time

USER = "aulo-spike"
MARKER = "aulo-spike-marker"


def main() -> int:
    if os.geteuid() != 0:
        os.execvp("sudo", ["sudo", "-n", sys.executable, *sys.argv])
    caller = os.environ.get("SUDO_USER") or "ubuntu"
    display = free_display()
    cleanup(display)
    try:
        subprocess.run(
            [
                "useradd",
                "--create-home",
                "--shell",
                "/usr/sbin/nologin",
                USER,
            ],
            check=True,
        )
        home = subprocess.check_output(["getent", "passwd", USER], text=True).split(":")[5]
        os.chmod(home, 0o700)
        marker = os.path.join(home, "marker.txt")
        subprocess.run(
            ["sudo", "-u", USER, "tee", marker],
            input=MARKER.encode(),
            check=True,
            stdout=subprocess.DEVNULL,
        )
        os.chmod(marker, 0o600)
        file_denied = other_cannot_read(caller, marker)
        proc = start_xvfb(display)
        socket_path = f"/tmp/.X11-unix/X{display}"
        wait_for(socket_path)
        os.chmod(socket_path, 0o700)
        socket_denied = other_cannot_connect(caller, display)
        owner_ok = owner_can_connect(display)
        print(f"file_denied={file_denied}")
        print(f"socket_denied={socket_denied}")
        print(f"owner_connected={owner_ok}")
        print(f"display=:{display}")
        if file_denied and socket_denied and owner_ok:
            return 0
        return 1
    finally:
        cleanup(display)


def free_display() -> int:
    for number in range(90, 110):
        if not os.path.exists(f"/tmp/.X11-unix/X{number}"):
            return number
    raise SystemExit("no free X display")


def start_xvfb(display: int) -> subprocess.Popen[bytes]:
    return subprocess.Popen(
        [
            "sudo",
            "-u",
            USER,
            "Xvfb",
            f":{display}",
            "-nolisten",
            "tcp",
            "-ac",
            "-screen",
            "0",
            "64x64x8",
        ],
        start_new_session=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )


def wait_for(path: str) -> None:
    for _ in range(50):
        if os.path.exists(path):
            return
        time.sleep(0.1)
    raise SystemExit(f"Xvfb did not create {path}")


def other_cannot_read(user: str, path: str) -> bool:
    code = (
        "import sys\n"
        "p = sys.argv[1]\n"
        "try:\n"
        "    open(p, 'rb').read()\n"
        "except PermissionError:\n"
        "    sys.exit(0)\n"
        "sys.exit(1)\n"
    )
    result = subprocess.run(
        ["sudo", "-u", user, sys.executable, "-c", code, path],
        check=False,
    )
    return result.returncode == 0


def other_cannot_connect(user: str, display: int) -> bool:
    code = (
        "import socket, sys\n"
        "s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)\n"
        "try:\n"
        "    s.connect('/tmp/.X11-unix/X' + sys.argv[1])\n"
        "except PermissionError:\n"
        "    sys.exit(0)\n"
        "sys.exit(1)\n"
    )
    result = subprocess.run(
        ["sudo", "-u", user, sys.executable, "-c", code, str(display)],
        check=False,
    )
    return result.returncode == 0


def owner_can_connect(display: int) -> bool:
    code = (
        "import socket, sys\n"
        "s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)\n"
        "s.settimeout(2)\n"
        "s.connect('/tmp/.X11-unix/X' + sys.argv[1])\n"
        "sys.exit(0)\n"
    )
    result = subprocess.run(
        ["sudo", "-u", USER, sys.executable, "-c", code, str(display)],
        check=False,
    )
    return result.returncode == 0


def cleanup(display: int) -> None:
    if subprocess.run(
        ["id", USER],
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    ).returncode == 0:
        subprocess.run(["pkill", "-u", USER], check=False, stderr=subprocess.DEVNULL)
        for _ in range(50):
            left = subprocess.run(["pgrep", "-u", USER], check=False, stdout=subprocess.DEVNULL)
            if left.returncode != 0:
                break
            time.sleep(0.1)
        subprocess.run(["userdel", "-r", USER], check=False, stderr=subprocess.DEVNULL)
    socket_path = f"/tmp/.X11-unix/X{display}"
    if os.path.exists(socket_path):
        os.remove(socket_path)


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except subprocess.CalledProcessError as err:
        print(f"command failed: {err.cmd!r} exit {err.returncode}", file=sys.stderr)
        raise SystemExit(1) from None
