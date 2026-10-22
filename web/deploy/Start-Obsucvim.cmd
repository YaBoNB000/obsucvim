@echo off
rem Kept for backwards compatibility. Start-Obsucvim.bat is the real launcher.
call "%~dp0Start-Obsucvim.bat" %*
exit /b %ERRORLEVEL%
