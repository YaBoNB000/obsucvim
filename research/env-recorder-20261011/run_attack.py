#!/usr/bin/env python3
"""Generate + run the recorder/replay attack against an obsucvim artifact."""
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
LUAU = ROOT / ".tools/bin/luau"
TEMPLATE = HERE / "replay_attack.luau"


def long_string(text: str) -> str:
    for level in range(1, 64):
        eq = "=" * level
        if "]" + eq + "]" not in text:
            return f"[{eq}[\n{text}]{eq}]"
    raise ValueError("delimiter")


def main():
    artifact = Path(sys.argv[1])
    label = sys.argv[2] if len(sys.argv) > 2 else artifact.stem
    harness = HERE / "work" / f"attack_{label}.luau"
    harness.parent.mkdir(exist_ok=True)
    harness.write_text(
        TEMPLATE.read_text().replace("@TARGET@", long_string(artifact.read_text()))
    )
    run = subprocess.run(
        [str(LUAU), str(harness)], capture_output=True, timeout=120
    )
    out = run.stdout.decode(errors="replace")
    print(out)
    if run.stderr:
        print("STDERR:", run.stderr.decode(errors="replace")[:400])
    return run.returncode


if __name__ == "__main__":
    sys.exit(main())
