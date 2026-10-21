# Obsucvim Web Windows x64 deployment package

Download `Obsucvim-Web-Windows-x64.zip`, verify it with the adjacent `.sha256`, extract on Windows Server 2019 x64, then start with `Start-Obsucvim.cmd`.

The bundle contains Node.js 22.22.3 x64, the Rust/WASI engine, a single-file UI, Nodemailer and PowerShell startup scripts. SMTP defaults to `smtp.163.com:465`, sender `obsucvim@163.com`; configure `SMTP_PASS` as the 163 client authorization code in a private server environment to enable real email. Without it, the UI stays in clearly labeled demo-mail mode. See the included `EMAIL-SETUP.md` and `README-Windows.md`.

The login and registration screen uses a single centered panel with responsive motion effects; reduced-motion preferences are respected.

## Registration and CAPTCHA

- Email addresses are normalized to lowercase and can register one account only.
- Each IP can complete one new registration. Existing accounts have no historical IP record and cannot be backfilled.
- Login/registration use a server-generated, one-time drag-the-piece CAPTCHA. It raises the cost of simple automation but is not proof against advanced computer vision or AI.
- If placed behind a reverse proxy, set `OBSUCVIM_TRUSTED_PROXY_IPS` to the exact proxy peer address and ensure the proxy supplies the real client IP in `X-Forwarded-For`; otherwise visitors may share one apparent IP.
- The public `/api/health` diagnostic endpoint has been removed. SMTP mode is reported only in the service startup console.

Daily quota remains 5 successful obfuscations per account. The package and its API paths were exercised in the Linux Node/WASI sandbox; physical Windows Server 2019 and external 163 SMTP delivery were not tested here.
