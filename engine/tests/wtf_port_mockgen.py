#!/usr/bin/env python3
"""Mock-harness generator for wtf_port_test.sh (research harness, not a
production artifact). Embeds the canary artifact verbatim in a cloned
environment plus one mock fragment; the test drives the result."""
import sys
from pathlib import Path


def long_string(text: str) -> str:
    for level in range(1, 64):
        eq = "=" * level
        if "]" + eq + "]" not in text:
            return f"[{eq}[\n{text}]{eq}]"
    raise ValueError("no collision-free long-string delimiter")


def main():
    mock_name, fragment_path, artifact_path, out_path = sys.argv[1:5]
    fragment = Path(fragment_path).read_text()
    artifact = Path(artifact_path).read_text()
    harness = f"""-- GENERATED mock harness: {mock_name}
local TARGET = {long_string(artifact)}
local BASE = getfenv(0)
local ENV = {{}}
for k, v in pairs(BASE) do ENV[k] = v end
ENV._G = ENV
local function copylib(name)
    local real = BASE[name]
    local c = {{}}
    for k, v in pairs(real) do c[k] = v end
    ENV[name] = c
    return c, real
end
{fragment}
local main = loadstring(TARGET, "canary")
setfenv(main, ENV)
main()
"""
    Path(out_path).write_text(harness)


if __name__ == "__main__":
    main()
