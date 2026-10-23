#!/usr/bin/env node
/**
 * Which obfuscator build is actually deployed?
 *
 *   node web/tools/verify-engine-version.mjs [path/to/obsucvim.wasm]
 *
 * The v15 output for a fixed input + fixed seed is deterministic, so the engine
 * build can be identified from its bytes. Known anchors:
 *
 *   R067    (2026-10-11, full noise density, bind-executor default)
 *           print("suda"), --preset v15 --env-check --bind-executor --bind-env roblox --seed 67067
 *           229,422 B  sha256 3775f3b557ca4c95ee16d9d013f290d735b6c022af23ac17f1d0b2339fc689f6
 *
 *   af7fc32 (2026-10-19, "reduce output size 232KB -> 212KB")
 *           same input/seed
 *           212,137 B  sha256 01099ddbb04eb2608ca230b3b7e4ed1af476d2f9afa3a78f43024caa0eda51da
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
    name: "R067",
    note: "full noise density (pre size-reduction), bind-executor default",
    size: 229422,
    sha256: "3775f3b557ca4c95ee16d9d013f290d735b6c022af23ac17f1d0b2339fc689f6",
  },
  {
    name: "af7fc32",
    note: "size reduction: noise halved, junk_n 2->1, obf_num 40->28 bit",
    size: 212137,
    sha256: "01099ddbb04eb2608ca230b3b7e4ed1af476d2f9afa3a78f43024caa0eda51da",
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
      "--preset", "v15", "--env-check", "--bind-executor", "--bind-env", "roblox",
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
  console.log(`probe    : print("suda") --env-check --bind-executor --bind-env roblox --seed 67067`);
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
