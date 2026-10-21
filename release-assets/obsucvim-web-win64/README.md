# Obsucvim Web Windows x64 deployment package

Download `Obsucvim-Web-Windows-x64.zip`, verify it with the adjacent `.sha256`, extract on Windows Server 2019 x64, then start with `Start-Obsucvim.cmd`.

The bundle includes Node.js 22.22.3 x64, the Rust/WASI engine, the single-file UI, Nodemailer and PowerShell startup scripts. SMTP defaults to `smtp.163.com:465`, sender `obsucvim@163.com`; configure `SMTP_PASS` as the 163 client authorization code in a private server environment. See the included `EMAIL-SETUP.md` and `README-Windows.md`.

## Security controls

- Browser sessions use server-side 7-day `HttpOnly; SameSite=Strict` cookies; session tokens are never returned to JavaScript or accepted as Bearer credentials. Existing sessions from the old localStorage-token version are invalidated once; users sign in again.
- On HTTPS, sessions also use `Secure`, and responses include one-year HSTS. When TLS terminates at a reverse proxy, configure its exact IP in `OBSUCVIM_TRUSTED_PROXY_IPS`; the Windows launcher defaults to loopback proxies.
- Responses include a hash-based CSP for the built inline script, `X-Frame-Options: DENY` and `frame-ancestors 'none'`.
- The server-generated slider CAPTCHA is one-use and limited to four image challenges per client IP per minute (plus the general auth limit).
- Public `/api/health` diagnostics are removed. Email and IP registration uniqueness remain server-enforced.

HSTS is learned only after an HTTPS response; first-visit protection requires domain preloading if desired. The API and package integrity were tested in the Linux Node/WASI sandbox; physical Windows Server 2019 and external 163 SMTP were not available for testing.
