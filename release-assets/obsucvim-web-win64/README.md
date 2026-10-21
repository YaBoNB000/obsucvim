# Obsucvim Web Windows x64 deployment package (163 SMTP edition)

Download `Obsucvim-Web-Windows-x64.zip`, verify with the adjacent `.sha256`, extract on Windows Server 2019 x64, then double-click `Start-Obsucvim.cmd`.

This bundle includes Node.js 22.22.3 x64, the Rust/WASI engine, static UI, Nodemailer, and PowerShell startup scripts. Daily quota is 5 per account. SMTP defaults to `smtp.163.com:465`, sender `obsucvim@163.com`; set `SMTP_PASS` to the 163 mailbox client authorization code in the server's private environment to enable real mail. Without it, registration remains in clearly labeled demo-mail mode. See `EMAIL-SETUP.md` and `README-Windows.md` inside the ZIP.

Verified in the development sandbox via Node/WASI: static UI and image serve; SMTP delivery path and email verification confirmed using a local SMTP test server; quota 5 is server-enforced (sixth request returns 429). A physical Windows Server 2019 host and external 163 SMTP were not available for testing.
