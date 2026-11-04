@echo off
chcp 65001 >nul
cd /d "%~dp0"
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\publish-menu.ps1" %*
set ERR=%ERRORLEVEL%
echo.
pause
exit /b %ERR%
