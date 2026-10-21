#!/usr/bin/env node
/* WASI Preview 1 runner for obsucvim. The parent server supplies a JSON
 * descriptor; only the per-request work directory is preopened to the CLI. */
import { WASI } from "node:wasi";
import { readFileSync } from "node:fs";

const cfg = JSON.parse(process.argv[2] || "{}");
if (!cfg.wasmPath || !cfg.workDir || !Array.isArray(cfg.args)) {
  console.error("bad wasi-runner config");
  process.exit(2);
}

try {
  const wasm = await WebAssembly.compile(readFileSync(cfg.wasmPath));
  const wasi = new WASI({
    version: "preview1",
    args: ["obsucvim", ...cfg.args],
    env: {},
    preopens: { "/work": cfg.workDir },
    stdin: 0,
    stdout: 1,
    stderr: 2,
    returnOnExit: true,
  });
  const instance = await WebAssembly.instantiate(wasm, {
    wasi_snapshot_preview1: wasi.wasiImport,
  });
  process.exitCode = wasi.start(instance);
} catch (e) {
  console.error(`WASI_ENGINE_ERROR:${e instanceof Error ? e.message : "unknown"}`);
  process.exitCode = 1;
}
