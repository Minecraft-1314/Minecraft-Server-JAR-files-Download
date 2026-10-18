@echo off
chcp 65001 >nul
cd /d "%~dp0"
where pwsh >nul 2>nul
if not errorlevel 1 (
  pwsh -NoProfile -ExecutionPolicy Bypass -File "%~dp0build.ps1" %*
) else (
  powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0build.ps1" %*
)
set "CODE=%ERRORLEVEL%"
if not "%CODE%"=="0" pause
exit /b %CODE%
