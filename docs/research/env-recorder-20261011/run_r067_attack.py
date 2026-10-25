#!/usr/bin/env python3
"""Drive the R067 attack harness against a bound-state artifact.

Usage: run_r067_attack.py <artifact> <tier:half|smart> <mode:direct|replay> [label]
"""
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = next((q for q in HERE.parents if (q / 'engine' / 'Cargo.toml').is_file()), HERE.parent.parent)
LUAU = ROOT / ".tools/bin/luau"
MOCK = ROOT / "engine/tests/wtf_port_roblox_mock.luau"
TEMPLATE = HERE / "r067_attack.luau"


def long_string(text: str) -> str:
    for level in range(1, 64):
        eq = "=" * level
        if "]" + eq + "]" not in text:
            return f"[{eq}[\n{text}]{eq}]"
    raise ValueError("delimiter")


def main() -> int:
    artifact = Path(sys.argv[1])
    tier = sys.argv[2] if len(sys.argv) > 2 else "smart"
    mode = sys.argv[3] if len(sys.argv) > 3 else "direct"
    label = sys.argv[4] if len(sys.argv) > 4 else f"{tier}_{mode}"
    mock = MOCK.read_text().replace("@BREAK@", "none")
    # the mock runs INSIDE BUILD(): strip nothing, it is plain top-level code
    harness = (
        TEMPLATE.read_text()
        .replace("@TARGET@", long_string(artifact.read_text()))
        .replace("@TIER@", tier)
        .replace("@MODE@", mode)
        .replace("\t--@MOCK@\n", mock)
    )
    out = HERE / "work" / f"r067_{label}.luau"
    out.parent.mkdir(exist_ok=True)
    out.write_text(harness)
    print(f"### harness: {out} (tier={tier} mode={mode} artifact={artifact.name})")
    run = subprocess.run([str(LUAU), str(out)], capture_output=True, timeout=180)
    text = run.stdout.decode(errors="replace")
    print(text)
    if run.stderr:
        print("STDERR:", run.stderr.decode(errors="replace")[:600])
    return run.returncode


if __name__ == "__main__":
    sys.exit(main())
