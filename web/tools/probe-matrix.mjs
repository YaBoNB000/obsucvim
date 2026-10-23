#!/usr/bin/env node
/**
 * Flag matrix probe for the WASI engine.
 *
 *   node tools/probe-matrix.mjs <source.luau> [seeds...]
 *
 * The engine sometimes dies with SIGSEGV (rc=139) without writing output. Whether
 * that happens depends on the input *and* the flags, so when a new template gets
 * injected we probe combinations instead of guessing.
 */
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, statSync, writeFileSync, copyFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const TOOLS = path.dirname(new URL(import.meta.url).pathname);
const WASM = path.join(TOOLS, "obsucvim.wasm");
const RUNNER = path.join(TOOLS, "wasi-runner.mjs");

const source = process.argv[2];
if (!source) {
  console.error("usage: node tools/probe-matrix.mjs <source.luau> [seeds...]");
  process.exit(2);
}
const seeds = process.argv.slice(3).length ? process.argv.slice(3) : ["1", "2", "3", "4"];

const BASE = ["--preset", "v15", "--env-check", "--no-bind-executor"];
const COMBOS = {
  default: ["--minify", "--vm-seal-strings"],
  noflatten: ["--minify", "--vm-seal-strings", "--no-flatten"],
  nojunk: ["--minify", "--vm-seal-strings", "--no-junk"],
  nobody: ["--minify", "--vm-seal-strings", "--no-body"],
  nonumbers: ["--minify", "--vm-seal-strings", "--no-numbers"],
  nomangle: ["--minify", "--vm-seal-strings", "--no-mangle"],
  nostrings: ["--minify", "--vm-seal-strings", "--no-strings"],
  minimal: ["--no-minify", "--no-mangle", "--no-strings", "--no-flatten", "--no-junk", "--no-body", "--no-numbers"],
  nolua: ["--minify", "--vm-seal-strings", "--dialect", "luau"],
};

const work = mkdtempSync(path.join(tmpdir(), "obsucvim-probe-"));
copyFileSync(source, path.join(work, "input.luau"));

const text = readFileSync(source, "utf8");
console.log(`source: ${source} (${Buffer.byteLength(text)} bytes)\n`);

const rows = [];
for (const [name, flags] of Object.entries(COMBOS)) {
  let ok = 0;
  const failures = [];
  for (const seed of seeds) {
    const out = `out_${name}_${seed}.lua`;
    const cfg = {
      wasmPath: WASM,
      workDir: work,
      args: [...BASE, ...flags, "--seed", String(seed), "/work/input.luau", `/work/${out}`],
    };
    const cfgPath = path.join(work, `cfg_${name}_${seed}.json`);
    writeFileSync(cfgPath, JSON.stringify(cfg));
    const target = path.join(work, out);
    rmSync(target, { force: true });
    const proc = spawnSync(process.execPath, [RUNNER, readFileSync(cfgPath, "utf8")], {
      encoding: "utf8",
      timeout: 300000,
    });
    let size = 0;
    try { size = statSync(target).size; } catch { size = 0; }
    if (proc.status === 0 && size > 0) ok += 1;
    else failures.push(`${seed}:${proc.status === null ? "killed" : proc.status}`);
  }
  rows.push({ name, ok, total: seeds.length, failures });
  console.log(`${name.padEnd(10)} ${String(ok).padStart(2)}/${seeds.length}  ${failures.length ? "fails=" + failures.join(",") : ""}`);
}

rmSync(work, { recursive: true, force: true });
const best = rows.filter((row) => row.ok === row.total).map((row) => row.name);
console.log(`\nstable combos: ${best.length ? best.join(", ") : "none"}`);
process.exit(best.length ? 0 : 1);
