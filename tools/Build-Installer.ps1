# Construit l'installateur Windows de NovyDeck (Inno Setup) à partir de target\release.
# Usage : powershell -ExecutionPolicy Bypass -File tools\Build-Installer.ps1 -Version 0.8.9 [-Build]
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [switch]$Build,
    [string]$Iscc
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root
if ($Build) {
    cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw "Compilation release en échec" }
}
foreach ($exe in "novydeck.exe", "novydeck-mcp.exe") {
    if (-not (Test-Path "target\release\$exe")) { throw "target\release\$exe absent : lancer avec -Build" }
}
if (-not $Iscc) {
    $candidates = @(
        (Get-Command iscc -ErrorAction SilentlyContinue).Source,
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles(x86)}\Inno Setup 7\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 7\ISCC.exe",
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe",
        "$env:LOCALAPPDATA\Programs\Inno Setup 7\ISCC.exe",
        "C:\Tools\InnoSetup\ISCC.exe"
    ) | Where-Object { $_ -and (Test-Path $_) }
    $Iscc = $candidates | Select-Object -First 1
}
if (-not $Iscc) { throw "ISCC.exe introuvable : installer Inno Setup (https://jrsoftware.org/isinfo.php) ou passer -Iscc" }
$out = Join-Path $root "target\release-dist"
New-Item -ItemType Directory -Force $out | Out-Null
$numeric = ($Version -replace "[^0-9.].*$", "").Trim(".")
& $Iscc "/DAppVersion=$Version" "/DAppNumVersion=$numeric" "/DSourceDir=$root\target\release" "/DRepoDir=$root" "/DOutputDir=$out" "$root\tools\installer\NovyDeck.iss"
if ($LASTEXITCODE -ne 0) { throw "Inno Setup a échoué ($LASTEXITCODE)" }
$setup = Join-Path $out "NovyDeck-$Version-Setup.exe"
$hash = (Get-FileHash $setup -Algorithm SHA256).Hash
"$hash  NovyDeck-$Version-Setup.exe" | Set-Content -Encoding ascii "$setup.sha256"
Write-Host "Installateur prêt : $setup"
Write-Host "SHA-256 : $hash"
