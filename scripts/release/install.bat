@echo off
setlocal enabledelayedexpansion
:: CodeWhale Windows installer
:: Copies codewhale.exe, codew.exe, and codewhale.bat to %USERPROFILE%\bin (single binary, no codewhale-tui.exe)

set "BIN_DIR=%USERPROFILE%\bin"
set "SCRIPT_DIR=%~dp0"

if not exist "%BIN_DIR%" mkdir "%BIN_DIR%"

for %%F in (codewhale.exe codew.exe codewhale.bat) do (
    if not exist "%SCRIPT_DIR%%%F" (
        echo ERROR: %%F is missing from !SCRIPT_DIR!
        echo Extract the whole release archive, then run install.bat from that folder.
        exit /b 1
    )
)

echo Installing codewhale to %BIN_DIR%...

copy /Y "%SCRIPT_DIR%codewhale.exe" "%BIN_DIR%\codewhale.exe" >nul
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to copy codewhale.exe
    exit /b 1
)

copy /Y "%SCRIPT_DIR%codew.exe" "%BIN_DIR%\codew.exe" >nul
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to copy codew.exe
    exit /b 1
)

copy /Y "%SCRIPT_DIR%codewhale.bat" "%BIN_DIR%\codewhale.bat" >nul
if %ERRORLEVEL% neq 0 (
    echo ERROR: Failed to copy codewhale.bat
    exit /b 1
)

echo.
echo Done. Commands installed to %BIN_DIR%.
echo.
echo Add %BIN_DIR% to your PATH:
echo   1. Open Start, search "environment variables"
echo   2. Click "Environment Variables..."
echo   3. Under "User variables", select "Path" and click "Edit"
echo   4. Click "New" and add: %BIN_DIR%
echo   5. Click OK, then restart your terminal
echo.
echo Or run this in PowerShell (no admin needed):
echo   [Environment]::SetEnvironmentVariable('Path', [Environment]::GetEnvironmentVariable('Path', 'User') + ';%BIN_DIR%', 'User')
echo.
echo Then run: codewhale
echo Double-click codewhale.bat (not codewhale.exe) to open Windows Terminal when it is installed.
