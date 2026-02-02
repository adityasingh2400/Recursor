@echo off
:: Recursor One-Click Installer for Windows
:: Double-click this file to install Recursor

echo.
echo   ========================================
echo        Installing Recursor...
echo   ========================================
echo.

:: Create directories
if not exist "%USERPROFILE%\.cursor\bin" mkdir "%USERPROFILE%\.cursor\bin"

:: Download using PowerShell
echo Downloading Recursor...
powershell -Command "& {Invoke-WebRequest -Uri 'https://github.com/adityasingh2400/Recursor/releases/latest/download/recursor-windows-x86_64.exe' -OutFile '%USERPROFILE%\.cursor\bin\recursor.exe'}" 2>nul

if %ERRORLEVEL% NEQ 0 (
    echo.
    echo No release found. Please install Rust and build from source:
    echo   https://rustup.rs
    echo.
    pause
    exit /b 1
)

:: Create hooks.json
echo Creating hooks configuration...
(
echo {
echo   "version": 1,
echo   "hooks": {
echo     "beforeSubmitPrompt": [
echo       { "command": "%USERPROFILE%\.cursor\bin\recursor.exe save" }
echo     ],
echo     "stop": [
echo       { "command": "%USERPROFILE%\.cursor\bin\recursor.exe restore" }
echo     ]
echo   }
echo }
) > "%USERPROFILE%\.cursor\hooks.json"

echo.
echo   ========================================
echo        Recursor installed successfully!
echo   ========================================
echo.
echo   Restart Cursor to activate.
echo.
pause
