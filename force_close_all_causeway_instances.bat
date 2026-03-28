@echo off
echo WARNING: This will kill ALL running Causeway instances across all projects.
echo To restart: reload your Claude Code session or VSCode window.
echo.
taskkill /F /IM causeway.exe >nul 2>&1
echo Done. All Causeway instances stopped.
pause
