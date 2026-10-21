# Obsucvim Web Windows x64 deployment package

Download `Obsucvim-Web-Windows-x64.zip`, verify it with the adjacent `.sha256`, extract on Windows Server 2019 x64, then start with `Start-Obsucvim.cmd`.

The bundle includes Node.js 22.22.3 x64, the Rust/WASI engine, the single-file UI, Nodemailer and PowerShell startup scripts. SMTP defaults to `smtp.163.com:465`, sender `obsucvim@163.com`; configure `SMTP_PASS` as the 163 client authorization code in a private server environment. See the included `EMAIL-SETUP.md` and `README-Windows.md`.

## Security controls

- Browser sessions use server-side 7-day `HttpOnly; SameSite=Strict` cookies; tokens are not returned to JavaScript. Old localStorage bearer sessions are invalidated once; users sign in again.
- HTTPS responses include Secure cookies and HSTS. Responses include hash-based CSP, `X-Frame-Options: DENY`, and `frame-ancestors 'none'`.
- Cloudflare Turnstile is optional: set `CLOUDFLARE_TURNSTILE_SITE_KEY`, `CLOUDFLARE_TURNSTILE_SECRET_KEY`, and `CLOUDFLARE_TURNSTILE_HOSTNAME` privately. The secret stays server-side and tokens are verified through Cloudflare Siteverify. With neither key, the local slider remains active; a partial configuration fails closed.
- CAPTCHA challenge issuance is limited to 4 per client IP per minute. Email and IP registration uniqueness are server-enforced. Public `/api/health` diagnostics are removed.

HSTS is learned after a successful HTTPS response; first-visit protection requires browser HSTS preloading if desired. The package/API paths were exercised in the Linux Node/WASI sandbox; physical Windows Server 2019, external 163 SMTP, and live Cloudflare verification were not tested here.
