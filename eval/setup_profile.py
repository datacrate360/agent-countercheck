"""Create the isolated `countercheck-eval` Hermes profile (idempotent).

Only touches ~/.hermes/profiles/countercheck-eval. The owner's other profiles, memory and skills
are never read or changed. The DeepSeek key is NOT written here: the owner adds
`DEEPSEEK_API_KEY=...` to that profile's .env by hand.

Usage: python3 eval/setup_profile.py [--profile countercheck-eval] [--model deepseek-v4-pro]
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from taskkit import COUNTERCHECK  # noqa: E402

SETTINGS = {
    # Every run starts from a blank slate: no memory, user profile or skill curation carried over.
    "memory.memory_enabled": "false",
    "memory.user_profile_enabled": "false",
    "curator.enabled": "false",
    # Arm B turns this on per run with HERMES_VERIFY_ON_STOP=1.
    "agent.verify_on_stop": "false",
    "hooks_auto_accept": "true",
}


def hermes(profile: str, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(["hermes", "-p", profile, *args], capture_output=True, text=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--profile", default="countercheck-eval")
    ap.add_argument("--provider", default="deepseek")
    ap.add_argument("--model", default="deepseek-v4-pro")
    a = ap.parse_args()

    home = Path(os.environ.get("HERMES_ROOT", Path.home() / ".hermes")) / "profiles" / a.profile
    if not home.exists():
        p = subprocess.run(["hermes", "profile", "create", a.profile], capture_output=True, text=True)
        if p.returncode != 0:
            print(p.stdout, p.stderr)
            return 1
        print(f"created profile {a.profile}")
    for key, value in {**SETTINGS, "model.provider": a.provider, "model.default": a.model}.items():
        p = hermes(a.profile, "config", "set", key, value)
        print(f"{'set ' if p.returncode == 0 else 'FAILED'} {key} = {value}")
        if p.returncode != 0:
            print(p.stdout[-500:], p.stderr[-500:])

    cfg = home / "config.yaml"
    text = cfg.read_text() if cfg.exists() else ""
    if "hermes-pre-verify" not in text:
        block = (
            "\nhooks:\n"
            "  pre_verify:\n"
            f"    - command: \"{COUNTERCHECK} hook hermes-pre-verify\"\n"
            "      timeout: 300\n"
        )
        if "\nhooks:" in text or text.startswith("hooks:"):
            print("config.yaml already has a hooks: block; add the pre_verify entry by hand:\n" + block)
            return 1
        cfg.write_text(text.rstrip("\n") + "\n" + block)
        print("added the Countercheck pre_verify hook")

    env = home / ".env"
    has_key = env.exists() and any(line.startswith("DEEPSEEK_API_KEY=") and len(line.strip()) > 18
                                   for line in env.read_text().splitlines())
    print(f"\nDEEPSEEK_API_KEY in {env}: {'present' if has_key else 'MISSING (add it by hand)'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
