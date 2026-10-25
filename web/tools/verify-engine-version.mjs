#!/usr/bin/env node
/**
 * Which obfuscator build is actually deployed?
 *
 *   node web/tools/verify-engine-version.mjs [path/to/obsucvim.wasm]
 *
 * The v15 output for a fixed input + fixed seed is deterministic, so the engine
 * build can be identified from its bytes. Known anchors:
 *
 * Probe: print("suda") with --preset v15 --env-check --bind-executor --seed 67067
 *
 *   R067.3  (2026-10-23, tier-2 no longer vetoes on implementation-specific values:
 *            Random exact floats and exact Luau parser wording became structural)
 *           230,177 B  sha256 70fbc8bb3f5303f4c1d4e279aaecea74bc649fb4068a8026d4aa9bf68c0e4c14
 *
 *   R067.2  (2026-10-23, precondition now resolves the executor env: getgenv()
 *            first, then _G — the R067.1 probe only read _G, so on Delta it
 *            reported every API as missing, including typeof)
 *           231,271 B  sha256 8f1b5e21a71e8886704ad79068d8114d386dddd4a9a4145af969b47a984575f5
 *
 *   R067.1  (2026-10-23, first precondition: reports missing APIs, but probes _G only)
 *           230,368 B  sha256 8b43132eb0297a9017689acfb3a1f80950b2fc269a62630dc4cb4483c014c8fb
 *
 *   R067    (2026-10-11, full noise density; junk_n=2, /12 /5 noise, obf_num 40 bit)
 *           229,422 B  sha256 3775f3b557ca4c95ee16d9d013f290d735b6c022af23ac17f1d0b2339fc689f6
 *
 *   af7fc32 (2026-10-19, "reduce output size"; noise halved, junk_n=1, obf_num 28 bit)
 *           229,900 B  sha256 12c2c05b572d5ede5d8992684ed4aa724096f9254b66d81563848b99fd91d638
 *
 * Anything else means the binary is a build we have no anchor for; the script
 * still reports the observed bytes/hash so it can be pinned deliberately.
 */
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync, statSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const TOOLS = path.dirname(new URL(import.meta.url).pathname);
const RUNNER = path.join(TOOLS, "wasi-runner.mjs");
const wasm = process.argv[2] || path.join(TOOLS, "obsucvim.wasm");

const ANCHORS = [
  {
    name: "R075-preview-a",
    note: "V19 M1-a 中间锚点：EVKEY 默认翻 On（自动档：v15+key-host⇒Bind，缺链⇒Off+警告）+ R6 指纹熔断（KeyEmitter 4 方言、_FQ 工厂随机构 15..18 参抖动+等价 decoy、载体三连格洗牌）；R3/R5 未落地，终态 R075 待钉；Off 自本轮起不再承诺 R073 逐字节恒等",
    size: 174163,
    sha256: "5ebefd82b3f1cc618ceef4636c4bcaf6eb0992a1685202d0d67e96f679f033c2",
  },
  {
    name: "R073",
    note: "体积收敛 R073：_mx 微解释器工厂化（5 份同源体→1 共享函数体+薄包装）+ HW2 别名表折叠；打印探针 231,571→171,881B，md 滚动方言/全 kind 注册不变",
    size: 171881,
    sha256: "b67a06e3f4ba620daaac0b81a3fb266cfb9f78fac9dda614052cbc7738024b9b",
  },
  {
    name: "R072",
    note: "V17 Phase-1b --super-insn 发射端接线收官（on/off 产物字节与 R069/R070/R071 完全相同，差异仅在 wasm 内部接线码）",
    size: 231571,
    sha256: "6c22c008c20883589a9f8ad3503151e2027880ac4454454a975842ba9731e11a",
  },
  {
    name: "R071",
    note: "V17 Phase-1a + --super-insn 演练门面（on/off 产物字节与 R069/R070 相同，差异仅在 wasm 内部规划码；与 R072 同锚点）",
    size: 231571,
    sha256: "6c22c008c20883589a9f8ad3503151e2027880ac4454454a975842ba9731e11a",
  },
  {
    name: "R070",
    note: "v16-a --key-host 固化恒开（--no-key-host 移除；产物字节与 R069 相同，差异仅在 CLI）",
    size: 231571,
    sha256: "6c22c008c20883589a9f8ad3503151e2027880ac4454454a975842ba9731e11a",
  },
  {
    name: "R069",
    note: "v16-a --key-host 翻默认（v15+executor 守卫自动注入 delta 宿主模型；--no-key-host 回退 = R067.5 字节）",
    size: 231571,
    sha256: "6c22c008c20883589a9f8ad3503151e2027880ac4454454a975842ba9731e11a",
  },
  {
    name: "R067.5",
    note: "tier-2 colon member-check uses bracket form (sanitize-rename universal poison fixed)",
    size: 231237,
    sha256: "f11a6cea20548681635f81e0d7e6c84987a12c0dbd5b4f8ab1ec81df72205c42",
  },
  {
    name: "R067.4",
    note: "exguard precheck via guard bindings (no bare getgenv); DTBL via LKP; CIDENT CENV-tolerant",
    size: 231496,
    sha256: "c1887865059cadb9b799f9defbdd132d4d685b720dd2e4787c05db1d8a79f653",
  },
  {
    name: "R067.3",
    note: "tier-2 structural (no exact PRNG floats / parser wording)",
    size: 230177,
    sha256: "70fbc8bb3f5303f4c1d4e279aaecea74bc649fb4068a8026d4aa9bf68c0e4c14",
  },
  {
    name: "R067.2",
    note: "precondition resolves getgenv() then _G (accurate missing list)",
    size: 231271,
    sha256: "8f1b5e21a71e8886704ad79068d8114d386dddd4a9a4145af969b47a984575f5",
  },
  {
    name: "R067.1",
    note: "R067 + hardened-build precondition (clear message instead of poison)",
    size: 230368,
    sha256: "8b43132eb0297a9017689acfb3a1f80950b2fc269a62630dc4cb4483c014c8fb",
  },
  {
    name: "R067",
    note: "full noise density, no precondition",
    size: 229422,
    sha256: "3775f3b557ca4c95ee16d9d013f290d735b6c022af23ac17f1d0b2339fc689f6",
  },
  {
    name: "af7fc32",
    note: "size reduction: noise halved, junk_n 2->1, obf_num 40->28 bit",
    size: 229900,
    sha256: "12c2c05b572d5ede5d8992684ed4aa724096f9254b66d81563848b99fd91d638",
  },
];

if (!existsSync(wasm)) {
  console.error(`engine not found: ${wasm}`);
  process.exit(2);
}

const work = mkdtempSync(path.join(tmpdir(), "obsucvim-verify-"));
try {
  writeFileSync(path.join(work, "in.lua"), 'print("suda")\n');
  const cfg = {
    wasmPath: path.resolve(wasm),
    workDir: work,
    args: [
      "--preset", "v15", "--env-check", "--bind-executor",
      "--seed", "67067", "/work/in.lua", "/work/out.lua",
    ],
  };
  const cfgPath = path.join(work, "cfg.json");
  writeFileSync(cfgPath, JSON.stringify(cfg));

  const proc = spawnSync(process.execPath, [RUNNER, readFileSync(cfgPath, "utf8")], {
    encoding: "utf8",
    timeout: 300000,
  });
  const outPath = path.join(work, "out.lua");
  if (proc.status !== 0 || !existsSync(outPath)) {
    console.error(`engine run failed (rc=${proc.status})`);
    console.error((proc.stderr || "").split("\n").filter((l) => l && !l.includes("ExperimentalWarning")).join("\n"));
    process.exit(1);
  }

  const bytes = statSync(outPath).size;
  const digest = createHash("sha256").update(readFileSync(outPath)).digest("hex");
  const match = ANCHORS.find((a) => a.size === bytes && a.sha256 === digest);

  console.log(`engine   : ${path.resolve(wasm)}`);
  console.log(`probe    : print("suda") --preset v15 --env-check --bind-executor --seed 67067`);
  console.log(`output   : ${bytes} bytes  ${digest}`);
  if (match) {
    console.log(`verdict  : ${match.name} — ${match.note}`);
  } else {
    console.log("verdict  : UNKNOWN build (no matching anchor)");
    console.log("known anchors:");
    for (const anchor of ANCHORS) {
      console.log(`  ${anchor.name.padEnd(8)} ${anchor.size} B  ${anchor.sha256}`);
    }
    process.exitCode = 1;
  }
} finally {
  rmSync(work, { recursive: true, force: true });
}
