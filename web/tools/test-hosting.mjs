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
  const { child, logs } = await startServer();
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
    /* ---------------- Phase 2: batch publish ---------------- */

    const second = await api("POST", "/api/auth/scripts", { name: "Second script", folderId });
    const secondId = second.data.script?.scriptId;
    const batchPublish = await api("POST", "/api/auth/publish/batch", {
      folderId, version: "v9", source: SOURCE, scriptIds: [secondId],
    });
    check("batch publish accepts a job list", batchPublish.status === 202 && batchPublish.data.count === 1,
      `status=${batchPublish.status} ${JSON.stringify(batchPublish.data).slice(0, 100)}`);
    if (batchPublish.status === 202) {
      for (const entry of batchPublish.data.jobs) {
        for (let i = 0; i < 100; i++) {
          const poll = await api("GET", `/api/publish/${entry.jobId}`);
          if (poll.data?.status !== "processing") break;
          await sleep(150);
        }
      }
      const rel = await api("GET", `/api/auth/releases?scriptId=${secondId}`);
      check("batch publish stored the release", (rel.data.releases || []).some((r) => r.version === "v9"));
    }

    /* ---------------- dual artifacts: hardened + compatible ---------------- */

    // 用一次全新的发布来验证（前面的发布已经被后续版本覆盖成「最新版本」）。
    const dual = await api("POST", "/api/publish", { scriptId, version: "vdual", source: SOURCE }, cookie);
    let dualJob = null;
    for (let i = 0; i < 200; i++) {
      const poll = await api("GET", `/api/publish/${dual.data.jobId}`);
      dualJob = poll.data;
      if (dualJob?.status !== "processing") break;
      await sleep(150);
    }
    const dualVariants = dualJob?.result?.variants ?? {};
    check("publish stores both variants", Boolean(dualVariants.safe) && Boolean(dualVariants.hard),
      JSON.stringify(Object.keys(dualVariants)));
    // 体积不是可靠指标（引擎的随机化本身就有 ±40 KB 抖动，同种子下加固层
    // 也可能让某些段变小）。这里断言的是服务端事实：两个变体各自的标签。
    check("variants are labelled by protection level",
      dualVariants.safe?.label === "compatible" && dualVariants.hard?.label === "hardened",
      `${dualVariants.safe?.label} / ${dualVariants.hard?.label}`);

    const safeFetch = await api("POST", "/api/releases/fetch", { scriptId, token: validate.data.token, variant: "safe" });
    const hardFetch = await api("POST", "/api/releases/fetch", { scriptId, token: validate.data.token, variant: "hard" });
    check("compatible variant fetchable", safeFetch.data?.variant === "safe" && safeFetch.data.code.length > 100);
    check("hardened variant fetchable", hardFetch.data?.variant === "hard" && hardFetch.data.code.length > 100);
    check("variants differ byte-wise", safeFetch.data.sha256 !== hardFetch.data.sha256);
    check("variant hashes match the job report",
      safeFetch.data.sha256 === dualVariants.safe.sha256 &&
      hardFetch.data.sha256 === dualVariants.hard.sha256,
      `${String(safeFetch.data.sha256).slice(0, 8)}/${String(dualVariants.safe?.sha256).slice(0, 8)}`);
    check("hardened payload differs from the compatible one",
      hardFetch.data.code.length !== safeFetch.data.code.length,
      `${safeFetch.data.code.length} vs ${hardFetch.data.code.length}`);

    const noVariant = await api("POST", "/api/releases/fetch", { scriptId, token: validate.data.token });
    check("fetch defaults to the compatible variant", noVariant.data?.variant === "safe");

    const bogusVariant = await api("POST", "/api/releases/fetch", {
      scriptId, token: validate.data.token, variant: "nonsense",
    });
    check("unknown variant falls back to compatible", bogusVariant.data?.variant === "safe");

    /* ---------------- attestation is always served with the loader ---------------- */

    const loaderText = await fetch(`${BASE}/files/${scriptId}.lua`).then((r) => r.text());
    check("served loader carries attestation", loaderText.includes("Obsucvim attestation")
      && loaderText.includes("OBS_Attested") && loaderText.includes("OBS_Attest"));
    check("attestation is ahead of the licence call",
      loaderText.indexOf("Obsucvim attestation") < loaderText.indexOf("/api/licenses/validate"));
    check("loader forwards the attested verdict",
      loaderText.includes("attested = attestedFlag") && loaderText.includes("attestScore"));

    const attestedCall = await api("POST", "/api/licenses/validate", {
      scriptId, key: licenseKey, hwid: "hwid-test-0123456789", nonce: "9".repeat(32),
      attested: true, attestScore: 22, attestTotal: 25, attestMissing: "debug.getconstants,",
    });
    check("validate accepts attestation fields", attestedCall.data?.valid === true);
    check("attestation is echoed back as data", attestedCall.data?.runtime?.attested === true
      && attestedCall.data?.runtime?.attestScore === 22,
      JSON.stringify(attestedCall.data?.runtime ?? {}).slice(0, 90));

    const plainCall = await api("POST", "/api/licenses/validate", {
      scriptId, key: licenseKey, hwid: "hwid-test-0123456789", nonce: "8".repeat(32),
      attested: false, attestScore: 3, attestTotal: 25,
    });
    check("unattested hosts still validate (never blocked)", plainCall.data?.valid === true);

    const attestMetrics = await api("GET", "/api/auth/metrics?days=2");
    const attestDay = (attestMetrics.data.days || []).at(-1);
    check("metrics count attested and unattested runs",
      (attestDay?.attested ?? 0) >= 1 && (attestDay?.unanattested ?? 0) >= 1,
      JSON.stringify(attestDay ?? {}));

    /* ---------------- key hygiene on the wire ---------------- */

    const padded = await api("POST", "/api/licenses/validate", {
      scriptId, key: `  ${licenseKey}\n`, hwid: "  hwid-test-0123456789  ", nonce: "d".repeat(32),
    });
    check("server trims padded keys and hwids", padded.data?.valid === true,
      JSON.stringify({ valid: padded.data?.valid, code: padded.data?.code }));

    const upper = await api("POST", "/api/licenses/validate", {
      scriptId, key: licenseKey.toUpperCase(), hwid: "hwid-test-0123456789", nonce: "e".repeat(32),
    });
    check("server accepts uppercase hex keys", upper.data?.valid === true,
      JSON.stringify({ valid: upper.data?.valid, code: upper.data?.code }));

    const shortKey = await api("POST", "/api/licenses/validate", {
      scriptId, key: "BAfjuLxndwTvMBNiCyqMsXMaTcOqXpcr", hwid: "hwid-test-0123456789", nonce: "f".repeat(32),
    });
    check("wrong-shaped key gets KEY_INVALID, not BAD_REQUEST",
      shortKey.status === 400 && shortKey.data?.code === "KEY_INVALID", shortKey.data?.code);

    const badScript = await api("POST", "/api/licenses/validate", {
      scriptId: "nope", key: licenseKey, hwid: "hwid-test-0123456789", nonce: "a".repeat(32),
    });
    check("bad script id gets SCRIPT_ID_INVALID", badScript.data?.code === "SCRIPT_ID_INVALID");

    const badNonce = await api("POST", "/api/licenses/validate", {
      scriptId, key: licenseKey, hwid: "hwid-test-0123456789", nonce: "!!",
    });
    check("bad nonce gets NONCE_INVALID", badNonce.data?.code === "NONCE_INVALID");

    /* ---------------- Phase 2: matrix stats / batch ops / audit ---------------- */

    const listed = await api("GET", "/api/auth/scripts");
    const mine = (listed.data.scripts || []).find((s2) => s2.scriptId === scriptId);
    check("script list carries stats", mine && mine.releaseCount >= 1 && Boolean(mine.latestVersion) &&
      typeof mine.licenseCount === "number", JSON.stringify({ v: mine?.latestVersion, n: mine?.licenseCount }));

    const batchLicenses = await api("POST", "/api/auth/licenses", { folderId, count: 3, label: "batch" });
    const batchIds = (batchLicenses.data.licenses || []).map((entry) => entry.license.licenseId);
    check("batch issue returns ids", batchIds.length === 3);

    const batchBan = await api("POST", "/api/auth/licenses/batch", {
      action: "ban", licenseIds: batchIds, reason: "chargeback", banDays: 1,
    });
    check("batch ban applies to every key", batchBan.status === 200 && batchBan.data.count === 3 &&
      batchBan.data.licenses.every((l) => l.banned === true));

    const batchUnban = await api("POST", "/api/auth/licenses/batch", { action: "unban", licenseIds: batchIds });
    check("batch unban clears bans", batchUnban.status === 200 &&
      batchUnban.data.licenses.every((l) => !l.banned));

    const batchReset = await api("POST", "/api/auth/licenses/batch", { action: "reset-hwid", licenseIds: batchIds });
    check("batch reset-hwid works", batchReset.status === 200 && batchReset.data.count === 3);

    const badBatch = await api("POST", "/api/auth/licenses/batch", { action: "ban", licenseIds: ["lic_nope"] });
    check("batch rejects foreign/unknown ids", badBatch.status === 400);

    const audit = await api("GET", "/api/auth/audit?limit=20");
    const actions = (audit.data.entries || []).map((e) => e.action);
    check("audit log records operations", audit.status === 200 &&
      actions.includes("licenses.ban") && actions.includes("licenses.unban"));

    /* ---------------- Phase 3: check_key codes + library ---------------- */

    const goodCheck = await api("POST", "/api/key/check", { scriptId, key: licenseKey, hwid: "hwid-test-0123456789" });
    check("check_key returns KEY_VALID", goodCheck.data?.code === "KEY_VALID" &&
      typeof goodCheck.data?.data?.total_executions === "number");

    const badFormat = await api("POST", "/api/key/check", { scriptId, key: "nope", hwid: "hwid-test-0123456789" });
    check("check_key reports KEY_INVALID", badFormat.data?.code === "KEY_INVALID");

    const wrongScript = await api("POST", "/api/key/check", { scriptId: "scr_" + "0".repeat(24), key: licenseKey, hwid: "hwid-test-0123456789" });
    check("check_key reports SCRIPT_ID_INCORRECT", wrongScript.data?.code === "SCRIPT_ID_INCORRECT");

    const noHwid = await api("POST", "/api/key/check", { scriptId, key: licenseKey });
    check("check_key reports INVALID_EXECUTOR", noHwid.data?.code === "INVALID_EXECUTOR");

    const lib = await fetch(`${BASE}/library.lua`).then((r) => r.text());
    check("library.lua served", /function api.check_key/.test(lib) && /api.script_id/.test(lib) &&
      lib.includes("/api/key/check") && lib.includes("/files/"));

    const validateRuntime = await api("POST", "/api/licenses/validate", {
      scriptId, key: importedKey, hwid: "hwid-test-abcdef012345", nonce: "c".repeat(32),
    });
    check("runtime exposes expiresAt and scriptVersion",
      validateRuntime.data?.runtime && "expiresAt" in validateRuntime.data.runtime &&
      Boolean(validateRuntime.data.runtime.scriptVersion),
      JSON.stringify(validateRuntime.data?.runtime ?? {}).slice(0, 120));

    /* ---------------- Phase 4: metrics / webhook log ---------------- */

    const metrics = await api("GET", "/api/auth/metrics?days=3");
    const today = (metrics.data.days || []).at(-1);
    check("metrics aggregate validations", metrics.status === 200 && today && today.validations >= 1,
      JSON.stringify(today ?? {}));

    const deliveries = await api("GET", "/api/auth/webhooks/deliveries");
    check("delivery log endpoint responds", deliveries.status === 200 && Array.isArray(deliveries.data.entries));

    /* ---------------- Phase 5: protected webhook ---------------- */

    const hook = await api("POST", "/api/auth/webhooks", {
      folderId, name: "join log", url: "https://discord.com/api/webhooks/1/abc",
      template: JSON.stringify({ content: "player {{name}} on %SCRIPT_NAME% from %CLIENT_IP%" }),
      sanitizers: { name: "[A-Za-z0-9_]{3,20}" },
    });
    const webhookId = hook.data?.webhook?.webhookId;
    check("protected webhook created", hook.status === 201 && Boolean(webhookId));
    check("webhook url is not echoed back", !JSON.stringify(hook.data).includes("discord.com"));

    const badHookUrl = await api("POST", "/api/auth/webhooks", {
      folderId, name: "evil", url: "http://127.0.0.1:9000/x",
      template: "{}", sanitizers: {},
    });
    check("non-discord webhook url rejected", badHookUrl.status === 400);

    const badRegex = await api("POST", "/api/auth/webhooks", {
      folderId, name: "bad regex", url: "https://discord.com/api/webhooks/1/abc",
      template: "{}", sanitizers: { name: "([a-z" },
    });
    check("invalid sanitizer regex rejected", badRegex.status === 400);

    const sessionForHook = validateRuntime.data?.token;
    const sent = await api("POST", "/api/webhooks/send", {
      token: sessionForHook, webhookId, values: { name: "Player_1" },
    });
    check("protected send is attempted with a valid session", sent.status === 200 || sent.status === 502,
      `${sent.status}/${JSON.stringify(sent.data).slice(0, 80)}`);

    const sanitizedOut = await api("POST", "/api/webhooks/send", {
      token: sessionForHook, webhookId, values: { name: "<script>bad" },
    });
    check("sanitizer blocks hostile values", sanitizedOut.status === 400 &&
      sanitizedOut.data.error === "SANITIZE_FAILED");

    const badToken = await api("POST", "/api/webhooks/send", { token: "lst_nope", webhookId, values: {} });
    check("send rejects unknown session", badToken.status === 401);

    /* ---------------- Phase 6: external check API ---------------- */

    const app = await api("POST", "/api/auth/external-apps", { name: "my launcher" });
    const appId = app.data?.app?.appId;
    const appSecret = app.data?.app?.secret;
    check("external app created with one-time secret", app.status === 201 && Boolean(appId) && Boolean(appSecret));

    const syncRes = await fetch(`${BASE}/external/sync`).then((r) => r.json());
    check("external sync returns server time", typeof syncRes.st === "number" && Array.isArray(syncRes.nodes));

    const crypto = await import("node:crypto");
    const sig = (nonce, key, clientTime, hwid) =>
      crypto.createHmac("sha256", appSecret).update([nonce, key, clientTime, hwid].join("|")).digest("hex");
    const nonce = "abcdef0123456789";
    const clientTime = String(syncRes.st);
    const extOk = await fetch(
      `${BASE}/external/check_key?by=${appId}&key=${encodeURIComponent(licenseKey)}`,
      { headers: {
          "content-type": "application/json",
          clienttime: clientTime,
          clientnonce: nonce,
          clienthwid: "hwid-test-0123456789",
          externalsignature: sig(nonce, licenseKey, clientTime, "hwid-test-0123456789"),
        } },
    ).then((r) => r.json());
    check("external check validates a good key", extOk.code === "KEY_VALID" && Boolean(extOk.signature),
      JSON.stringify(extOk).slice(0, 120));

    const extBadSig = await fetch(`${BASE}/external/check_key?by=${appId}&key=${encodeURIComponent(licenseKey)}`,
      { headers: { clienttime: clientTime, clientnonce: nonce, clienthwid: "x", externalsignature: "deadbeef" } })
      .then((r) => r.json());
    check("external check rejects a bad signature", extBadSig.code === "SECURITY_ERROR");

    const extStale = await fetch(`${BASE}/external/check_key?by=${appId}&key=${encodeURIComponent(licenseKey)}`,
      { headers: {
          clienttime: String(Number(syncRes.st) - 3_600_000), clientnonce: nonce, clienthwid: "x",
          externalsignature: sig(nonce, licenseKey, String(Number(syncRes.st) - 3_600_000), "x"),
        } })
      .then((r) => r.json());
    check("external check rejects stale client time", extStale.code === "TIME_ERROR");

    const extWrongApp = await fetch(`${BASE}/external/check_key?by=app_nope&key=${encodeURIComponent(licenseKey)}`)
      .then((r) => r.json());
    check("unknown external app is refused", extWrongApp.code === "SECURITY_ERROR");

  } catch (error) {
    console.error("--- server log tail ---");
    console.error(logs.slice(-40).join(""));
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
