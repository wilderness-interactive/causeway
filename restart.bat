@echo off
taskkill /F /IM causeway.exe >nul 2>&1
echo Causeway stopped. It will restart automatically on next tool call.
