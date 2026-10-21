# Obsucvim Web Windows x64 deployment package

Download `Obsucvim-Web-Windows-x64.zip`, verify it with the adjacent `.sha256`, extract on Windows Server 2019 x64, then start with `Start-Obsucvim.cmd`.

The bundle includes Node.js 22.22.3 x64, the Rust/WASI engine, the single-file UI, Nodemailer and PowerShell startup scripts. SMTP defaults to `smtp.163.com:465`, sender `obsucvim@163.com`; configure `SMTP_PASS` as the 163 client authorization code in a private server environment. See `EMAIL-SETUP.md` and `README-Windows.md`.

## Security and licensing prototype

- Browser sessions use server-side `HttpOnly; SameSite=Strict` cookies. HTTPS responses enable Secure cookies and HSTS; a hash-based CSP and anti-framing headers are included.
- Optional Cloudflare Turnstile is verified server-side. With no Turnstile keys the local slider remains active; partial configuration fails closed.
- Per-customer script keys are returned only at creation and stored server-side as SHA-256 hashes. Validation supports expiry, revocation, and a best-effort first-use HWID binding (client-provided HWID is spoofable).
- The current release includes project/key management and `/api/licenses/validate`; it does **not yet** include source upload, hosted release delivery, or a remote execution loader. See `SCRIPT-KEYS.md` for the prototype API and Luau validation example.

The sandbox tests covered the key issue/validate/revoke/HWID flow and package integrity. Real Cloudflare Siteverify, physical Windows Server 2019, and external 163 SMTP were not tested here.
