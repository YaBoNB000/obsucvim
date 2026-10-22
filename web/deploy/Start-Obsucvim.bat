@echo off
rem ==========================================================================
rem  Obsucvim Web - one-file launcher for Windows Server 2019 x64
rem
rem  Double-click for the menu. From a script pass flags:
rem    Start-Obsucvim.bat /local     bind 127.0.0.1 only
rem    Start-Obsucvim.bat /public    bind 0.0.0.0 (behind an HTTPS proxy)
rem    Start-Obsucvim.bat /smtp      ask for the 163 auth code, then start
rem    Start-Obsucvim.bat /check     run preflight checks only
rem    Start-Obsucvim.bat /update    run the verified self-updater
rem    Start-Obsucvim.bat /open      open the browser when the server is up
rem    Start-Obsucvim.bat /help      show usage
rem
rem  Port override: Start-Obsucvim.bat /local 9090
rem  Everything runs from the bundled runtime\node.exe. Nothing to install.
rem ==========================================================================
setlocal EnableExtensions EnableDelayedExpansion
title Obsucvim Web
cd /d "%~dp0"

set "ROOT=%~dp0"
if "%ROOT:~-1%"=="\" set "ROOT=%ROOT:~0,-1%"
set "NODE=%ROOT%\runtime\node.exe"
set "SERVER=%ROOT%\tools\engine-server.mjs"
set "WASM=%ROOT%\tools\obsucvim.wasm"
set "DIST=%ROOT%\dist"
set "DATA=%ROOT%\data"
set "UPDATER=%ROOT%\tools\self-update.mjs"

set "PORT=8787"
if not "%OBS_PORT%"=="" set "PORT=%OBS_PORT%"

set "BIND=public"
set "ASKSECRET=0"
set "CHECKONLY=0"
set "DOUPDATE=0"
set "OPENBROWSER=0"
set "SKIPMENU=0"
set "RC=0"

set "HASCHOICE=1"
where choice >nul 2>&1
if errorlevel 1 set "HASCHOICE=0"

rem ---------------------------------------------------------------- arguments
rem Each flag goes through its own label so "shift" never runs inside a block.
:parseargs
if "%~1"=="" goto :dispatch
if /i "%~1"=="/local"  goto :arg_local
if /i "%~1"=="/public" goto :arg_public
if /i "%~1"=="/smtp"   goto :arg_smtp
if /i "%~1"=="/check"  goto :arg_check
if /i "%~1"=="/update" goto :arg_update
if /i "%~1"=="/open"   goto :arg_open
if /i "%~1"=="/help"   goto :usage
if /i "%~1"=="/?"      goto :usage

rem a bare number is treated as a port, anything non-numeric is ignored
set "NONNUM="
for /f "delims=0123456789" %%A in ("%~1") do set "NONNUM=1"
if "!NONNUM!"=="" set "PORT=%~1"
shift
goto :parseargs

:arg_local
set "BIND=local"
set "SKIPMENU=1"
shift
goto :parseargs

:arg_public
set "BIND=public"
set "SKIPMENU=1"
shift
goto :parseargs

:arg_smtp
set "ASKSECRET=1"
set "BIND=public"
set "SKIPMENU=1"
shift
goto :parseargs

:arg_check
set "CHECKONLY=1"
set "SKIPMENU=1"
shift
goto :parseargs

:arg_update
set "DOUPDATE=1"
set "SKIPMENU=1"
shift
goto :parseargs

:arg_open
set "OPENBROWSER=1"
shift
goto :parseargs

:usage
echo.
echo   Obsucvim Web launcher
echo.
echo     no argument    interactive menu
echo     /local         bind 127.0.0.1 only, safe for testing
echo     /public        bind 0.0.0.0, only behind an HTTPS reverse proxy
echo     /smtp          prompt for the 163 client auth code, then start
echo     /check         verify files and port, do not start
echo     /update        run the verified self-updater
echo     /open          open the browser once the server is up
echo     /help          this text
echo.
echo   Port override:   Start-Obsucvim.bat /local 9090
echo.
if "%SKIPMENU%"=="0" pause
goto :done

rem ------------------------------------------------------------------- router
:dispatch
if "%SKIPMENU%"=="0" goto :menu
goto :checkfiles

rem ---------------------------------------------------------------- preflight
:checkfiles
set "BROKEN="
if not exist "%NODE%"            set "BROKEN=1"
if not exist "%SERVER%"          set "BROKEN=1"
if not exist "%WASM%"            set "BROKEN=1"
if not exist "%DIST%\index.html" set "BROKEN=1"
if not "!BROKEN!"=="" goto :broken

if not exist "%DATA%" mkdir "%DATA%" >nul 2>&1

if "%CHECKONLY%"=="1" goto :status
if "%DOUPDATE%"=="1" goto :doupdate
goto :start

rem ------------------------------------------------------------------- start
:start
cls
echo.
echo   ============================================================
echo    Obsucvim Web
call :builddate
echo   ============================================================
echo.
echo   Port        %PORT%
echo   Data        %DATA%
echo.

if /i "%BIND%"=="local" (
  set "HOST=127.0.0.1"
  echo   URL         http://localhost:%PORT%
) else (
  set "HOST=0.0.0.0"
  echo   URL         http://localhost:%PORT%
  echo   Network     http://THIS-SERVER-IP:%PORT%
  echo   WARNING     Only expose this behind HTTPS. Plain HTTP sends the
  echo               login and session cookie in clear text.
)
echo.

if "%ASKSECRET%"=="1" call :readsecret
if "!SMTP_PASS!"=="" (
  echo   Mail        DEMO mode, verification codes are shown in the UI
) else (
  echo   Mail        SMTP enabled for !SMTP_USER!
)
echo   Stop        close this window or press Ctrl+C
echo.

call :checkport
if not "!PORTBUSY!"=="" goto :portbusy

rem engine binary is intentionally absent so the bundled WASI engine is used
set "OBSUCVIM_BIN=%ROOT%\tools\obsucvim.exe"
set "OBSUCVIM_WASM=%WASM%"
set "OBSUCVIM_WEB_ROOT=%DIST%"
set "OBSUCVIM_STATE_FILE=%DATA%\state.json"
set "OBSUCVIM_DATA_DIR=%DATA%"
if "!OBSUCVIM_TRUSTED_PROXY_IPS!"=="" set "OBSUCVIM_TRUSTED_PROXY_IPS=127.0.0.1,::1"
if "!SMTP_USER!"=="" set "SMTP_USER=obsucvim@163.com"
if "!SMTP_FROM!"=="" set "SMTP_FROM=!SMTP_USER!"
if "!SMTP_HOST!"=="" set "SMTP_HOST=smtp.163.com"
if "!SMTP_PORT!"=="" set "SMTP_PORT=465"
set "PORT=%PORT%"

if "%OPENBROWSER%"=="1" start "" cmd /c "timeout /t 3 >nul & start http://localhost:%PORT%"

"%NODE%" "%SERVER%"
set "RC=!ERRORLEVEL!"
echo.
if "!RC!"=="0" (
  echo   Obsucvim stopped.
) else (
  echo   Obsucvim exited with code !RC!. See the messages above.
)
echo.
if "%SKIPMENU%"=="0" pause
goto :done

rem -------------------------------------------------------------------- menu
:menu
set "CHECKONLY=0"
set "DOUPDATE=0"
set "ASKSECRET=0"
cls
echo.
echo   ============================================================
echo    Obsucvim Web
call :builddate
echo   ============================================================
echo.
echo     [1]  Start - this machine only       http://localhost:%PORT%
echo     [2]  Start - allow network access    behind an HTTPS proxy
echo     [3]  Start with 163 email verification
echo     [4]  Check installation, do not start
echo     [5]  Update to the latest build
echo     [6]  Open the data folder
echo     [7]  Open the install folder
echo     [8]  Exit
echo.
set "PICK="
if "%HASCHOICE%"=="1" (
  choice /c 12345678 /n /m "   Select [1-8]: "
  set "PICK=!ERRORLEVEL!"
) else (
  set /p "PICK=   Select [1-8]: "
)
if "!PICK!"=="1" goto :menu_local
if "!PICK!"=="2" goto :menu_public
if "!PICK!"=="3" goto :menu_smtp
if "!PICK!"=="4" goto :menu_check
if "!PICK!"=="5" goto :menu_update
if "!PICK!"=="6" goto :menu_data
if "!PICK!"=="7" goto :menu_root
if "!PICK!"=="8" goto :done
goto :menu

:menu_local
set "BIND=local"
set "SKIPMENU=0"
goto :checkfiles

:menu_public
set "BIND=public"
set "SKIPMENU=0"
goto :checkfiles

:menu_smtp
set "ASKSECRET=1"
set "BIND=public"
set "SKIPMENU=0"
goto :checkfiles

:menu_check
set "CHECKONLY=1"
set "SKIPMENU=0"
goto :checkfiles

:menu_update
set "DOUPDATE=1"
set "SKIPMENU=0"
goto :checkfiles

:menu_data
if not exist "%DATA%" mkdir "%DATA%" >nul 2>&1
start "" explorer "%DATA%"
goto :menu

:menu_root
start "" explorer "%ROOT%"
goto :menu

rem ----------------------------------------------------------------- helpers
:builddate
if exist "%ROOT%\BUILD-INFO.txt" for /f "usebackq tokens=1,* delims=:" %%A in ("%ROOT%\BUILD-INFO.txt") do (
  if /i "%%A"=="Build date" echo    Build       %%B
)
goto :eof

:checkport
set "PORTBUSY="
for /f "tokens=5" %%P in ('netstat -ano -p TCP ^| findstr /R /C:":%PORT% .*LISTENING"') do set "PORTBUSY=%%P"
goto :eof

:readsecret
echo   Paste the 163 client authorization code. Input is hidden.
echo   Generate it in 163 mail settings under POP3/SMTP/IMAP.
echo   It is NOT your mailbox password.
echo.
rem cmd cannot mask input, so a tiny throwaway PowerShell script does it.
rem Writing it to a file avoids every quoting and paren trap of -Command.
set "SECRETPS=%TEMP%\obsucvim_read_secret.ps1"
if not exist "%TEMP%" set "SECRETPS=%ROOT%\data\obsucvim_read_secret.ps1"
> "%SECRETPS%" echo $s = Read-Host -AsSecureString 'Auth code'
>>"%SECRETPS%" echo if ($null -eq $s) { exit 1 }
>>"%SECRETPS%" echo $b = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($s)
>>"%SECRETPS%" echo try { [Runtime.InteropServices.Marshal]::PtrToStringBSTR($b) } finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($b) }
set "SMTP_PASS="
for /f "usebackq delims=" %%P in (`powershell -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%SECRETPS%"`) do set "SMTP_PASS=%%P"
del "%SECRETPS%" >nul 2>&1
if "!SMTP_PASS!"=="" echo   Nothing entered, staying in demo mail mode.
echo.
goto :eof

:status
cls
echo.
echo   Files
if exist "%NODE%"            (echo     OK       runtime\node.exe)        else (echo     MISSING  runtime\node.exe)
if exist "%SERVER%"          (echo     OK       tools\engine-server.mjs)  else (echo     MISSING  tools\engine-server.mjs)
if exist "%WASM%"            (echo     OK       tools\obsucvim.wasm)      else (echo     MISSING  tools\obsucvim.wasm)
if exist "%DIST%\index.html" (echo     OK       dist\index.html)          else (echo     MISSING  dist\index.html)
echo.
echo   Port %PORT%
call :checkport
if "!PORTBUSY!"=="" (echo     free) else (echo     IN USE by PID !PORTBUSY!)
echo.
echo   Data
if exist "%DATA%\state.json" (
  for %%S in ("%DATA%\state.json") do echo     state.json   %%~zS bytes
) else (
  echo     no state.json yet, created on first run
)
if exist "%DATA%\releases" (
  for /f %%C in ('dir /b /ad "%DATA%\releases" 2^>nul ^| find /c /v ""') do echo     releases     %%C script folders
)
echo.
echo   Mail
if "!SMTP_PASS!"=="" (echo     demo mode, codes shown in the UI) else (echo     SMTP configured for !SMTP_USER!)
echo.
if "%SKIPMENU%"=="0" pause
if "%SKIPMENU%"=="0" goto :menu
goto :done

:doupdate
if not exist "%UPDATER%" goto :noupdater
cls
echo.
echo   Updating. Program files are replaced, data\ is kept and backed up.
echo.
"%NODE%" "%UPDATER%" --dir "%ROOT%"
set "RC=!ERRORLEVEL!"
echo.
if not "!RC!"=="0" (
  echo   Update failed, nothing was changed. See the message above.
) else (
  echo   Update finished. Restart with Start-Obsucvim.bat
)
echo.
if "%SKIPMENU%"=="0" pause
if "%SKIPMENU%"=="0" goto :menu
goto :done

:portbusy
echo.
echo   Port %PORT% is already taken by PID !PORTBUSY!.
echo   That is usually an Obsucvim instance that is still running.
echo   Close that window, or use another port:
echo       Start-Obsucvim.bat /local 9090
echo.
if "%SKIPMENU%"=="0" pause
if "%SKIPMENU%"=="0" goto :menu
goto :done

:broken
echo.
echo   This installation is incomplete:
echo.
if not exist "%NODE%"            echo     missing  runtime\node.exe
if not exist "%SERVER%"          echo     missing  tools\engine-server.mjs
if not exist "%WASM%"            echo     missing  tools\obsucvim.wasm
if not exist "%DIST%\index.html" echo     missing  dist\index.html
echo.
echo   Re-extract the release zip over this folder and try again.
echo   Extracting does not touch data\ or your settings.
echo.
if "%SKIPMENU%"=="0" pause
goto :done

:noupdater
echo.
echo   tools\self-update.mjs is missing.
echo   Re-extract the newest release zip, or copy that file in manually.
echo.
if "%SKIPMENU%"=="1" goto :done
pause
goto :menu

:done
endlocal
exit /b 0
