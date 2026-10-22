#!/usr/bin/env node
/**
 * Regression test for the engine crash retry.
 *
 * The WASI obfuscator occasionally dies with SIGSEGV (rc=139) and produces no
 * output; the same input usually succeeds on the next run with a fresh seed.
 * This replaces the engine with a stub binary that fails a configurable number
 * of times, so the retry path is exercised deterministically and fast.
 *
 *   node tools/test-engine-retry.mjs
 */
import { spawn } from "node:child_process";
import { chmodSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const WORK = mkdtempSync(path.join(tmpdir(), "obsucvim-retry-"));
const PORT = 19000 + Math.floor(Math.random() * 900);
const BASE = `http://127.0.0.1:${PORT}`;
const STATE = path.join(WORK, "state.json");
const COUNTER = path.join(WORK, "counter");
const MODE = path.join(WORK, "mode");

// Stub engine: reads MODE ("fail" | "fail_once"), counts calls in COUNTER.
// Native mode expects the generated code on stdout.
const STUB = path.join(WORK, "engine-stub.sh");
writeFileSync(STUB, `#!/bin/sh
count=$(cat "${COUNTER}" 2>/dev/null || echo 0)
count=$((count + 1))
echo "$count" > "${COUNTER}"
mode=$(cat "${MODE}" 2>/dev/null || echo fail_once)
if [ "$mode" = "fail" ]; then
  echo "engine crashed" >&2
  exit 139
fi
if [ "$count" -lt 2 ]; then
  echo "engine crashed" >&2
  exit 139
fi
printf '%s' "local x = 1; return 'STUB-ARTIFACT-0123456789'"
exit 0
`);
chmodSync(STUB, 0o755);

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const results = [];
function check(name, ok, detail = "") {
  results.push({ name, ok });
  console.log(`${ok ? "PASS" : "FAIL"} ${name}${detail ? ` | ${detail}` : ""}`);
}

async function call(method, url, body, cookie) {
  const hasBody = method !== "GET" && method !== "HEAD";
  const res = await fetch(BASE + url, {
    method,
    headers: {
      ...(hasBody ? { "content-type": "application/json" } : {}),
      ...(cookie ? { cookie } : {}),
    },
    ...(hasBody ? { body: JSON.stringify(body ?? {}) } : {}),
  });
  const text = await res.text();
  let data;
  try { data = JSON.parse(text); } catch { data = text; }
  return { status: res.status, data, setCookie: res.headers.get("set-cookie") };
}

async function main() {
  writeFileSync(COUNTER, "0");
  writeFileSync(MODE, "fail_once");
  const child = spawn(process.execPath, [path.join(ROOT, "tools", "engine-server.mjs")], {
    cwd: ROOT,
    env: {
      ...process.env,
      HOST: "127.0.0.1",
      PORT: String(PORT),
      OBSUCVIM_BIN: STUB,
      OBSUCVIM_STATE_FILE: STATE,
      OBSUCVIM_DATA_DIR: WORK,
      OBSUCVIM_PUBLIC_ORIGIN: "https://obsucvim.l2.ink",
      OBSUCVIM_TRUSTED_PROXY_IPS: "127.0.0.1",
      CLOUDFLARE_TURNSTILE_SITE_KEY: "",
      CLOUDFLARE_TURNSTILE_SECRET_KEY: "",
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  const logs = [];
  child.stdout.on("data", (d) => logs.push(String(d)));
  child.stderr.on("data", (d) => logs.push(String(d)));

  try {
    for (let i = 0; i < 100; i++) {
      try {
        await fetch(`${BASE}/`);
        break;
      } catch {
        await sleep(100);
      }
    }

    // register
    const cap = await call("POST", "/api/auth/captcha");
    await sleep(900);
    const answer = JSON.parse(readFileSync(STATE, "utf8")).captchas[cap.data.captchaId].targetX;
    const email = `retry${Date.now()}@example.com`;
    const reg = await call("POST", "/api/auth/register", {
      email, password: "correct-horse-battery",
      captchaId: cap.data.captchaId, captchaAnswer: String(answer),
    });
    const confirm = await call("POST", "/api/auth/register/confirm", { email, code: reg.data.demoCode });
    check("account ready", confirm.data?.ok === true);
    const cookie = confirm.setCookie.split(";")[0];

    const folder = await call("POST", "/api/auth/folders", { name: "Retry hub" }, cookie);
    const folderId = folder.data.folder.folderId;
    const script = await call("POST", "/api/auth/scripts", { name: "Retry script", folderId }, cookie);
    const scriptId = script.data.script.scriptId;

    const waitJob = async (jobId) => {
      for (let i = 0; i < 200; i++) {
        const poll = await call("GET", `/api/publish/${jobId}`, null, cookie);
        if (poll.data?.status !== "processing") return poll.data;
        await sleep(80);
      }
      return null;
    };

    // 1. one crash, then success -> job completes
    const first = await call("POST", "/api/publish", { scriptId, version: "v1", source: "print('x')\n" }, cookie);
    const job1 = await waitJob(first.data.jobId);
    check("publish survives a single engine crash", job1?.status === "completed",
      `${job1?.status}/${job1?.error ?? "ok"}`);
    check("artifact from the retried attempt is stored",
      (await call("GET", `/api/auth/releases?scriptId=${scriptId}`, null, cookie)).data.releases
        ?.some((r) => r.version === "v1") === true);

    // 2. always crashing -> ENGINE_CRASH, quota refunded, nothing stored
    writeFileSync(MODE, "fail");
    writeFileSync(COUNTER, "0");
    const before = (await call("GET", "/api/me", null, cookie)).data.quota.usedToday;
    const second = await call("POST", "/api/publish", { scriptId, version: "v2", source: "print('y')\n" }, cookie);
    const job2 = await waitJob(second.data.jobId);
    check("permanent crash reports ENGINE_CRASH", job2?.status === "failed" && job2?.error === "ENGINE_CRASH",
      `${job2?.status}/${job2?.error}`);
    check("crash detail mentions the attempt count",
      typeof job2?.detail === "string" && /3 attempts/.test(job2.detail), String(job2?.detail ?? "").slice(0, 90));
    check("engine was retried three times", readFileSync(COUNTER, "utf8").trim() === "3",
      `calls=${readFileSync(COUNTER, "utf8").trim()}`);
    const after = (await call("GET", "/api/me", null, cookie)).data.quota.usedToday;
    check("crash refunds the quota", before === after, `${before} -> ${after}`);
    check("failed publish leaves no release",
      !(await call("GET", `/api/auth/releases?scriptId=${scriptId}`, null, cookie)).data.releases
        ?.some((r) => r.version === "v2"));

    // 3. a syntax error must NOT be retried
    writeFileSync(MODE, "syntax");
    writeFileSync(COUNTER, "0");
    writeFileSync(STUB, `#!/bin/sh
echo "$(($(cat "${COUNTER}" 2>/dev/null || echo 0) + 1))" > "${COUNTER}"
echo "error: unexpected symbol near '='" >&2
exit 1
`);
    const third = await call("POST", "/api/publish", { scriptId, version: "v3", source: "print('z')\n" }, cookie);
    const job3 = await waitJob(third.data.jobId);
    check("syntax error fails fast without retrying",
      job3?.status === "failed" && job3?.error === "ENGINE_FAIL" && readFileSync(COUNTER, "utf8").trim() === "1",
      `${job3?.error} calls=${readFileSync(COUNTER, "utf8").trim()}`);
  } catch (error) {
    console.error("--- server log ---");
    console.error(logs.slice(-30).join(""));
    throw error;
  } finally {
    child.kill("SIGKILL");
    rmSync(WORK, { recursive: true, force: true });
  }

  const failed = results.filter((r) => !r.ok);
  console.log(`\n${results.length - failed.length}/${results.length} passed`);
  process.exit(failed.length ? 1 : 0);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
