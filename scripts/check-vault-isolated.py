#!/usr/bin/env python3
"""Exercise the compiled vault test in the project SDK against a private oo7.

Never connects to the desktop bus or reads its keyring. Build the test binary
inside the configured Flatpak SDK first, then pass its absolute path.
"""
import argparse
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    manifest = Path(__file__).resolve().parents[1] / "flatpak/com.rtosta.DriveSync.Devel.yml"
    version = re.search(r"^runtime-version: ['\"]?([^'\"\n]+)", manifest.read_text(), re.M).group(1)
    sdk = f"org.gnome.Sdk//{version}"
    with tempfile.TemporaryDirectory(prefix="drivesync-vault-") as temporary:
        root = Path(temporary)
        env = os.environ.copy()
        env.pop("CREDENTIALS_DIRECTORY", None)
        env.pop("ENCRYPTED_CREDENTIALS_DIRECTORY", None)
        for name, folder in [("XDG_DATA_HOME", "data"), ("XDG_CONFIG_HOME", "config"), ("XDG_CACHE_HOME", "cache"), ("XDG_RUNTIME_DIR", "runtime")]:
            directory = root / folder
            directory.mkdir(mode=0o700)
            env[name] = str(directory)
        env["DRIVESYNC_TEST_VAULT_ROOT"] = temporary
        env["DBUS_SESSION_BUS_ADDRESS"] = f"unix:path={root}/bus"
        bus = subprocess.Popen(["dbus-daemon", "--session", "--nofork", f"--address={env['DBUS_SESSION_BUS_ADDRESS']}"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        daemon = None
        try:
            for _ in range(100):
                if (root / "bus").exists():
                    break
                if bus.poll() is not None:
                    raise RuntimeError("Private D-Bus failed to start")
                time.sleep(0.05)
            # Only synthetic data is used, including the disposable login password.
            with (root / "daemon.log").open("w") as log:
                daemon = subprocess.Popen(["/usr/libexec/oo7-daemon", "--login"], env=env, stdin=subprocess.PIPE, stdout=log, stderr=subprocess.STDOUT)
                daemon.stdin.write(b"synthetic-isolated-vault-password\n")
                daemon.stdin.close()
                for _ in range(200):
                    query = subprocess.run(["busctl", "--user", "get-property", "org.freedesktop.secrets", "/org/freedesktop/secrets/aliases/default", "org.freedesktop.Secret.Collection", "Locked"], env=env, capture_output=True, text=True)
                    if query.returncode == 0 and query.stdout.strip() == "b false":
                        break
                    if daemon.poll() is not None:
                        raise RuntimeError("Isolated oo7 exited; desktop vault was not accessed")
                    time.sleep(0.05)
                else:
                    raise RuntimeError("Isolated oo7 did not provide an unlocked default collection")
                command = ["flatpak", "run", "--devel", "--socket=session-bus", f"--filesystem={root}", f"--filesystem={binary.parent}:ro"]
                for name in ["DBUS_SESSION_BUS_ADDRESS", "XDG_DATA_HOME", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_RUNTIME_DIR", "DRIVESYNC_TEST_VAULT_ROOT"]:
                    command.append(f"--env={name}={env[name]}")
                command += ["--command=env", sdk, f"DBUS_SESSION_BUS_ADDRESS={env['DBUS_SESSION_BUS_ADDRESS']}", str(binary), "auth::storage::tests::isolated_vault_roundtrip", "--exact", "--ignored", "--nocapture"]
                subprocess.run(command, env=env, check=True, timeout=300)
        finally:
            for process in [daemon, bus]:
                if process is not None and process.poll() is None:
                    process.terminate()
                    try:
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait()


if __name__ == "__main__":
    main()
