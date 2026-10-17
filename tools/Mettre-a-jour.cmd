@echo off
setlocal
set "DECKFORGE_UPDATE_DIR=%~dp0"
powershell.exe -NoProfile -Command "& ([scriptblock]::Create([IO.File]::ReadAllText((Join-Path $env:DECKFORGE_UPDATE_DIR 'Mettre-a-jour.ps1'))))"
endlocal
