#!/usr/bin/env node
/**
 * Hosting end-to-end tests (real HTTP against a real engine-server process).
 *
 *   node tools/test-hosting.mjs
 *
 * Covers the async publish job, progress polling, quota refund on failure,
 * per-account job limits, ownership checks, JSON export/import and the
 * heartbeat flag being baked into /files/<id>.lua.
 *
 * The captcha answer is read from the state file on purpose: this script is a
 * test harness, it never ships to production.
 */
import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const ROOT = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const PORT = Number(process.env.TEST_PORT || 18000 + Math.floor(Math.random() * 1500));
const BASE = `http://127.0.0.1:${PORT}`;
const WORK = mkdtempSync(path.join(tmpdir(), "obsucvim-hosting-"));
const STATE = path.join(WORK, "state.json");

const results = [];
function check(name, ok, detail = "") {
  results.push({ name, ok });
  console.log(`${ok ? "PASS" : "FAIL"} ${name}${detail ? ` | ${detail}` : ""}`);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function startServer() {
  const child = spawn(process.execPath, [path.join(ROOT, "tools", "engine-server.mjs")], {
    cwd: ROOT,
    env: {
      ...process.env,
      HOST: "127.0.0.1",
      PORT: String(PORT),
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
  child.on("exit", (code) => {
    if (code !== null && code !== 0) logs.push(`child exited early with ${code}`);
  });
  for (let i = 0; i < 100; i++) {
    try {
      // Any HTTP answer means the listener is up; "/" may be 503 when dist/ is absent.
      const res = await fetch(`${BASE}/`);
      if (res.status > 0) return { child, logs };
    } catch { /* not up yet */ }
    await sleep(100);
  }
  throw new Error(`server did not start:\n${logs.join("")}`);
}

function readState() {
  return JSON.parse(readFileSync(STATE, "utf8"));
}

let cookie = "";
async function api(method, url, body) {
  const hasBody = method !== "GET" && method !== "HEAD";
  const res = await fetch(BASE + url, {
    method,
    headers: {
      ...(hasBody ? { "content-type": "application/json" } : {}),
      ...(cookie ? { cookie } : {}),
    },
    ...(hasBody ? { body: JSON.stringify(body ?? {}) } : {}),
  });
  const setCookie = res.headers.get("set-cookie");
  if (setCookie) cookie = setCookie.split(";")[0];
  const text = await res.text();
  let data;
  try { data = JSON.parse(text); } catch { data = text; }
  return { status: res.status, data, headers: res.headers };
}

async function register(email) {
  const cap = await api("POST", "/api/auth/captcha");
  const captchaId = cap.data.captchaId;
  await sleep(800); // the server rejects answers younger than 700 ms
  const snapshot = readState();
  const captcha = (snapshot.captchas || {})[captchaId];
  if (!captcha) throw new Error(`captcha not in state: id=${captchaId} keys=${Object.keys(snapshot).join(",")} resp=${JSON.stringify(cap.data).slice(0, 200)}`);
  const targetX = captcha.targetX;
  const reg = await api("POST", "/api/auth/register", {
    email, nickname: "tester", password: "correct-horse-battery",
    captchaId, captchaAnswer: String(targetX),
  });
  if (!reg.data?.demoCode) throw new Error(`register failed: ${JSON.stringify(reg.data)}`);
  const confirm = await api("POST", "/api/auth/register/confirm", {
    email, code: reg.data.demoCode,
  });
  if (!confirm.data?.ok) throw new Error(`confirm failed: ${JSON.stringify(confirm.data)}`);
}

const SOURCE = `local Players = game:GetService("Players")
print("demo script")
return "ok"
`;

async function main() {
  const { child } = await startServer();
  const email = `test${Date.now()}@example.com`;

  try {
    await register(email);

    // ---- project + script
    const folder = await api("POST", "/api/auth/folders", { name: "Main hub" });
    const folderId = folder.data.folder?.folderId;
    check("folder created", (folder.status === 200 || folder.status === 201) && Boolean(folderId), `status=${folder.status}`);

    const script = await api("POST", "/api/auth/scripts", { name: "Demo script", folderId });
    const scriptId = script.data.script?.scriptId;
    check("script created", script.status === 201 && Boolean(scriptId));

    // ---- async publish
    const publish = await api("POST", "/api/publish", { scriptId, version: "v1", source: SOURCE });
    check("publish is accepted as a job", publish.status === 202 && /^pub_/.test(publish.data.jobId || ""),
      `status=${publish.status}`);
    const jobId = publish.data.jobId;

    const stages = new Set();
    let job = null;
    for (let i = 0; i < 200; i++) {
      const poll = await api("GET", `/api/publish/${jobId}`);
      if (poll.status !== 200) break;
      job = poll.data;
      stages.add(job.stage);
      if (job.status !== "processing") break;
      await sleep(150);
    }
    check("job completes", job?.status === "completed", JSON.stringify({ status: job?.status, error: job?.error }));
    check("job reports progress stages", stages.has("injecting") || stages.has("obfuscating") || stages.has("queued"),
      [...stages].join(","));
    check("job reports 100% when done", job?.progress === 100);
    check("job result carries sha256 and size", Boolean(job?.result?.sha256) && job?.result?.size > 1000);

    // ---- publish lands in releases and is fetchable
    const releases = await api("GET", `/api/auth/releases?scriptId=${scriptId}`);
    const rel = (releases.data.releases || []).find((r) => r.version === "v1");
    check("release listed after job", Boolean(rel) && rel.sha256 === job.result.sha256);

    const lic = await api("POST", "/api/auth/licenses", { folderId, label: "buyer" });
    const licenseKey = lic.data.licenseKey;
    check("license issued", typeof licenseKey === "string" && licenseKey.startsWith("osk_"));

    const validate = await api("POST", "/api/licenses/validate", {
      scriptId, key: licenseKey, hwid: "hwid-test-0123456789", nonce: "a".repeat(32),
    });
    check("issued key validates", validate.data?.valid === true, JSON.stringify(validate.data?.code || ""));
    const fetchRelease = await api("POST", "/api/releases/fetch", { scriptId, token: validate.data.token });
    check("artifact fetch matches published sha256", fetchRelease.data?.code &&
      (await import("node:crypto")).createHash("sha256").update(fetchRelease.data.code).digest("hex") === job.result.sha256);

    // ---- second concurrent job is refused while one is running is racy to test;
    // instead assert the limit constant is enforced for the *owner* by firing 3 at once.
    const burst = await Promise.all([
      api("POST", "/api/publish", { scriptId, version: "v2", source: SOURCE }),
      api("POST", "/api/publish", { scriptId, version: "v3", source: SOURCE }),
      api("POST", "/api/publish", { scriptId, version: "v4", source: SOURCE }),
    ]);
    const accepted = burst.filter((r) => r.status === 202).length;
    const busy = burst.filter((r) => r.status === 409 && r.data.error === "PUBLISH_BUSY").length;
    check("per-account concurrent job limit", accepted <= 2 && busy >= 1, `accepted=${accepted} busy=${busy}`);
    for (const r of burst) {
      if (r.status !== 202) continue;
      child.on("exit", (code) => {
    if (code !== null && code !== 0) logs.push(`child exited early with ${code}`);
  });
  for (let i = 0; i < 100; i++) {
        const p = await api("GET", `/api/publish/${r.data.jobId}`);
        if (p.data?.status !== "processing") break;
        await sleep(150);
      }
    }

    // ---- failure refunds quota and leaves no release behind
    const before = (await api("GET", "/api/me")).data.quota?.usedToday ?? 0;
    const bad = await api("POST", "/api/publish", { scriptId, version: "bad", source: "local = = =\nif then" });
    check("broken source accepted as job", bad.status === 202);
    let badJob = null;
    child.on("exit", (code) => {
    if (code !== null && code !== 0) logs.push(`child exited early with ${code}`);
  });
  for (let i = 0; i < 100; i++) {
      const p = await api("GET", `/api/publish/${bad.data.jobId}`);
      badJob = p.data;
      if (badJob?.status !== "processing") break;
      await sleep(150);
    }
    check("broken source fails the job", badJob?.status === "failed" && Boolean(badJob?.error),
      `${badJob?.status}/${badJob?.error}`);
    const after = (await api("GET", "/api/me")).data.quota?.usedToday ?? 0;
    check("failed publish refunds the quota", after === before, `before=${before} after=${after}`);
    const badRelease = await api("GET", `/api/auth/releases?scriptId=${scriptId}`);
    check("failed publish stores nothing", !(badRelease.data.releases || []).some((r) => r.version === "bad"));

    // ---- job ownership
    const strangerJob = await api("GET", "/api/publish/pub_doesnotexist");
    check("unknown job id is 404", strangerJob.status === 404 && strangerJob.data.error === "JOB_NOT_FOUND");

    // ---- export / import
    const json = await api("GET", `/api/auth/licenses/export.json?folderId=${folderId}`);
    const exported = JSON.stringify(json.data);
    check("json export returns metadata", json.status === 200 && json.data.count >= 1);
    check("json export never contains a key", !/osk_[0-9a-f]{16}/.test(exported));

    const imported = await api("POST", "/api/auth/licenses/import", {
      folderId,
      entries: [{ label: "migrated-1", keyDays: 30 }, { label: "migrated-2" }],
    });
    check("json import issues new keys", imported.status === 201 && imported.data.count === 2 &&
      imported.data.licenses.every((l) => l.licenseKey.startsWith("osk_")));
    const importedKey = imported.data.licenses[0].licenseKey;
    const importedValidate = await api("POST", "/api/licenses/validate", {
      scriptId, key: importedKey, hwid: "hwid-test-abcdef012345", nonce: "b".repeat(32),
    });
    check("imported key works", importedValidate.data?.valid === true);

    const badImport = await api("POST", "/api/auth/licenses/import", {
      folderId, entries: [{ label: "ok", keyDays: 5 }, { label: "bad", keyDays: 99999 }],
    });
    check("import rejects the whole batch when one entry is invalid", badImport.status === 400);

    // ---- heartbeat flag reaches the loader
    const off = await fetch(`${BASE}/files/${scriptId}.lua`).then((r) => r.text());
    check("loader has heartbeat off by default", /local HEARTBEAT = false/.test(off));
    const updated = await api("POST", "/api/auth/scripts/update", { scriptId, heartbeat: true });
    check("heartbeat can be enabled", updated.status === 200 && updated.data.script?.heartbeat === true);
    const on = await fetch(`${BASE}/files/${scriptId}.lua`).then((r) => r.text());
    check("loader reflects heartbeat flag", /local HEARTBEAT = true/.test(on));
    check("loader keeps the stable script id", on.includes(scriptId) && on.includes("https://obsucvim.l2.ink"));

    // ---- unauthenticated access
    const saved = cookie;
    cookie = "";
    const anon = await api("GET", `/api/publish/${jobId}`);
    check("job polling requires a session", anon.status === 401);
    cookie = saved;
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
