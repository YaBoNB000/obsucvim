# Obsucvim Web Windows x64 deployment package

Download `Obsucvim-Web-Windows-x64.zip`, verify with the adjacent `.sha256`, extract on Windows Server 2019 x64, then double-click `Start-Obsucvim.cmd`.

The bundle contains Node.js 22.22.3 x64, the Rust engine compiled to WASI Preview 1, the static web UI, Nodemailer, and PowerShell startup scripts. The email verification provider runs in demo mode until `SMTP_PASS` is configured privately on the server. See `README-Windows.md` inside the ZIP for setup and security notes.

Verified in the development sandbox via Node/WASI: static UI and assets return 200; registration and authenticated obfuscation work. A physical Windows Server 2019 runtime test was not available in this environment.
