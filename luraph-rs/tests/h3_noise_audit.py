#!/usr/bin/env python3
"""H3 噪声随机化审计。

验证六个构建期不变量：独立 seed 化 PRNG、真值分布采样、NOP 数量/形态、
活噪声、nil/decoy 槽随机，以及真实/噪声在高低数值域双向重叠。正式产物
不携带统计；仅在 LURAPH_NOISE_AUDIT 显式开启时由编译器写临时 manifest。
"""
import os
import pathlib
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
TOOL = ROOT / "target/release/luraph-rs"
TOOLS = pathlib.Path("/home/user/luraph/.tools/bin")
LUAU = TOOLS / "luau"
if not LUAU.exists():
    LUAU = pathlib.Path("/home/user/tools/bin/luau")

SRC = """local function fold(n)
    local s = 0
    for i = 1, n do
        if i % 2 == 0 then s = s + i * 3 else s = s - i end
    end
    return s
end
local t = {fold(18), fold(11), 7}
local function mix(x) return x + t[1] - t[3] end
print(mix(t[2]), t[1])
"""


def build(seed, src_path, out_path, audit_path):
    pathlib.Path(src_path).write_text(SRC)
    env = os.environ.copy()
    env["LURAPH_NOISE_AUDIT"] = str(audit_path)
    p = subprocess.run(
        [str(TOOL), "--preset", "v15", "--seed", str(seed),
         str(src_path), str(out_path)],
        capture_output=True, text=True, env=env,
    )
    if p.returncode != 0:
        raise RuntimeError(p.stderr[:500])
    return pathlib.Path(out_path).read_bytes(), parse_audit(audit_path)


def parse_audit(path):
    rows = []
    for line in pathlib.Path(path).read_text().splitlines():
        row = {}
        for item in line.split():
            key, value = item.split("=", 1)
            row[key] = int(value)
        rows.append(row)
    return sorted(rows, key=lambda row: row["fn"])


def run_luau(path):
    return subprocess.run([str(LUAU), str(path)], capture_output=True)


def pct(num, den):
    return 100.0 * num / max(1, den)


def main():
    if not TOOL.exists():
        print(f"missing release binary: {TOOL}", file=sys.stderr)
        return 2
    with tempfile.TemporaryDirectory() as td:
        td = pathlib.Path(td)
        src = td / "src.lua"
        out_a, out_b, out_c = td / "a.lua", td / "b.lua", td / "c.lua"
        a, sa = build(314159, src, out_a, td / "a.noise")
        b, sb = build(271828, src, out_b, td / "b.noise")
        c, sc = build(314159, src, out_c, td / "c.noise")

        deterministic = a == c and sa == sc
        seed_diff = a != b and [r["seed"] for r in sa] != [r["seed"] for r in sb]
        print("== H3 噪声随机化审计 ==")
        print(f"[0] 同 seed 产物+噪声统计确定: {'YES' if deterministic else 'NO'}")
        print(f"[1] 异 seed 独立噪声流变化    : {'YES' if seed_diff else 'NO'}")

        rows = sa + sb
        accounting = all(
            r["final"] == r["base"] + r["nop"] + r["active"]
            and r["nop"] >= 1 and r["active"] >= 1
            and 2 <= r["dead"] <= 4
            for r in rows
        )
        count_shapes_a = [(r["nop"], r["active"], r["final"] - r["base"]) for r in sa]
        count_shapes_b = [(r["nop"], r["active"], r["final"] - r["base"]) for r in sb]
        count_diff = count_shapes_a != count_shapes_b
        print(f"[2] NOP 数量随机 + 活噪声记账: "
              f"{'YES' if accounting and count_diff else 'NO'} "
              f"(A={count_shapes_a}, B={count_shapes_b})")

        real_n = sum(r["real_n"] for r in rows)
        real_hi = sum(r["real_hi"] for r in rows)
        noise_n = sum(r["noise_n"] for r in rows)
        noise_hi = sum(r["noise_hi"] for r in rows)
        # 两类都必须同时覆盖低区和高区；不要求比例相同，避免把审计写成
        # 可反向利用的固定分布模板。
        overlap = (
            0 < real_hi < real_n
            and 0 < noise_hi < noise_n
            and all(r["reg_hi"] >= 1 for r in rows)
        )
        print(f"[3] 真/噪声高低域双向重叠    : {'YES' if overlap else 'NO'} "
              f"(real_hi={pct(real_hi, real_n):.1f}%, "
              f"noise_hi={pct(noise_hi, noise_n):.1f}%)")

        slot_mix = all(r["nil"] + r["decoy"] >= 3 for r in rows)
        nil_total = sum(r["nil"] for r in rows)
        decoy_total = sum(r["decoy"] for r in rows)
        slot_diff = [(r["nil"], r["decoy"]) for r in sa] != [
            (r["nil"], r["decoy"]) for r in sb
        ]
        print(f"[4] nil/decoy 槽数量与比例随机: "
              f"{'YES' if slot_mix and slot_diff and nil_total and decoy_total else 'NO'} "
              f"(nil={nil_total}, decoy={decoy_total})")

        src_run = run_luau(src)
        run_a, run_b = run_luau(out_a), run_luau(out_b)
        runtime = (
            src_run.returncode == run_a.returncode == run_b.returncode == 0
            and src_run.stdout == run_a.stdout == run_b.stdout
        )
        print(f"[5] 双 seed 运行等价          : {'YES' if runtime else 'NO'} "
              f"(out={src_run.stdout!r})")

        template = (ROOT / "src/vmgen/template.rs").read_text()
        no_zero_reset = "PF_[{}].SA[{}] = 0" not in template
        nop_src = (ROOT / "src/vmgen/handlers/nop.rs").read_text()
        four_shapes = all(x in nop_src for x in ["a + b", "a < b", "local _p", "{a, c}"])
        print(f"[6] NOP 四形态且无全零回写   : "
              f"{'YES' if no_zero_reset and four_shapes else 'NO'}")

        ok = all([
            deterministic, seed_diff, accounting, count_diff, overlap,
            slot_mix, slot_diff, bool(nil_total), bool(decoy_total), runtime,
            no_zero_reset, four_shapes,
        ])
        print("\nH3 审计: " + ("PASS" if ok else "FAIL"))
        return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
