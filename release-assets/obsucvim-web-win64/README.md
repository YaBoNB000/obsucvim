# Obsucvim Web Windows x64 deployment package

Download `Obsucvim-Web-Windows-x64.zip`, verify it with the adjacent `.sha256`, extract on Windows Server 2019 x64, then start with `Start-Obsucvim.cmd`.

The bundle includes Node.js 22.22.3 x64, the Rust/WASI engine, the single-file UI, Nodemailer and PowerShell startup scripts. SMTP defaults to `smtp.163.com:465`, sender `obsucvim@163.com`; configure `SMTP_PASS` as the 163 client authorization code in a private server environment. See `EMAIL-SETUP.md` and `README-Windows.md`.

## Security and licensing prototype

- Browser sessions use server-side `HttpOnly; SameSite=Strict` cookies. HTTPS responses enable Secure cookies and HSTS; a hash-based CSP and anti-framing headers are included.
- Optional Cloudflare Turnstile is verified server-side. With no Turnstile keys the local slider remains active; partial configuration fails closed.
- Per-customer script keys are returned only at creation and stored server-side as SHA-256 hashes. Validation supports expiry, revocation, and a best-effort first-use HWID binding (client-provided HWID is spoofable).
- In-script license guard: `tools/inject-license.mjs` prepends `tools/license-guard.luau` to user source, filling in the public project id and validate URL. The guard reads the per-customer key from `getgenv().key`, requires an executor HWID and a POST-capable HTTP API, checks the server response (status, valid, scriptId, nonce) and fails closed. Obfuscate the guarded file so the check is protected with user code.
- Hardened licensing: keys are stored as HMAC digests, nonces are single-use (5 min), a 2-hour session token gates downloads, guess throttling is per client+project and never blocks a correct key, and revoking a key invalidates its tokens immediately.
- License-protected delivery: owners publish obfuscated artifacts via `POST /api/auth/releases`; `POST /api/releases/fetch` returns them only to a valid licence, and `tools/loader-template.luau` wires that flow. A modified loader still cannot download the artifact without a valid key.
- Web console now has a Hosting page: create projects, issue/revoke licence keys, reset device binding, and publish releases with per-version SHA-256.
- One-click pipeline: upload raw source in the console and the server injects the licence guard, obfuscates with v15 and stores the artifact. The raw source is never persisted.
- Key expiry has three modes: forever, fixed date, or `keyDays` counted from first activation (so keys can be stocked before sale).
- Project folders: a key belongs to a folder and unlocks every script inside it, so one key can cover a whole hub.
- Batch issue up to 100 keys per call, plus CSV export that contains metadata only — raw keys are never re-readable because only HMAC digests are stored.
- Optional execution-log and alert webhooks, restricted to https Discord hosts and rate limited so they cannot be abused as an SSRF relay.
- Customers get a public `#key` page to check their licence and, when the project enables it, self-reset the device binding on a cooldown.
- Published guards inject `OBS_IsPremium`, `OBS_UserNote`, `OBS_ScriptName`, `OBS_SecondsLeft` and `OBS_TotalExecutions` for UI gating. These are client-side values and can be tampered with, so never use them for security decisions.
- HWID is client-supplied and spoofable; treat binding as friction, not hardware attestation.

## Starting

Extract, then double-click `Start-Obsucvim.bat`. It shows a menu (local only / network / 163 mail / self-check / update / open folders) and accepts unattended flags: `/local`, `/public`, `/smtp`, `/check`, `/update`, `/open`, plus an optional port such as `/local 9090`. `Start-Obsucvim.cmd` forwards to the same launcher; `Start-Obsucvim.ps1` only adds firewall rules.

## Hosting (Luarmor-style)

- Every script gets a stable loader URL, `https://<your-domain>/files/<scriptId>.lua`. Users paste one `loadstring(game:HttpGet("..."))()` line and never change it; re-publishing swaps the content behind the same URL.
- Free for All can be enabled per script; keyless runs report `premium = false` so the script can split free and paid features.
- Licences support permanent and temporary bans. A ban also blocks the device that used the key, so a new key on the same machine is refused, and it returns an unban token the user can redeem.
- Licence lists are paginated with search and status filters.

## Upgrading

Run `Update-Obsucvim.ps1 -Sha256 "<hash>"` inside the install directory (or `runtime\node.exe tools\self-update.mjs --dir . --url <zip> --sha256 <hash>`). It verifies the zip hash and the internal manifest, backs up `data/`, replaces program files only, and leaves accounts/keys/releases untouched. See `README-UPDATE.md`.

The sandbox tests covered the key issue/validate/revoke/HWID flow and package integrity. Real Cloudflare Siteverify, physical Windows Server 2019, and external 163 SMTP were not tested here.
