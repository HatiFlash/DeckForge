# Script de génération de la distribution NovyDeck Windows
param(
    [string]$Version = "0.8.9",
    [string]$OutputDir = "target\release-dist"
)

$ErrorActionPreference = "Stop"
Write-Host "[1/4] Compilation en mode Release..." -ForegroundColor Cyan
cargo build --release --bin novydeck --bin novydeck-mcp
if ($LASTEXITCODE -ne 0) { throw 'Compilation release en échec' }

Write-Host "[2/4] Préparation de l'archive et des ressources documentaires..." -ForegroundColor Cyan
$DistFolder = Join-Path $OutputDir "NovyDeck-$Version-Windows"
if (Test-Path $DistFolder) {
    Remove-Item -Path $DistFolder -Recurse -Force
}
New-Item -ItemType Directory -Path $DistFolder -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $DistFolder "docs") -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $DistFolder "tools") -Force | Out-Null

Copy-Item -Path "target\release\novydeck.exe", "target\release\novydeck-mcp.exe", "README.md", "LICENSE", "THIRD-PARTY-NOTICES.txt" -Destination $DistFolder
$checksums = [ordered]@{}
foreach ($name in @('novydeck.exe', 'novydeck-mcp.exe')) {
    $checksums[$name] = (Get-FileHash -LiteralPath (Join-Path $DistFolder $name) -Algorithm SHA256).Hash
}
$checksums | ConvertTo-Json | Set-Content -Path (Join-Path $DistFolder 'sha256-files.json') -Encoding UTF8

# Copie intégrale de toute la documentation et des images associées pour éviter tout lien brisé
if (Test-Path "docs") {
    Copy-Item -Path "docs\*" -Destination (Join-Path $DistFolder "docs") -Recurse -Force
}
if (Test-Path "tools\Mettre-a-jour.ps1") {
    Copy-Item -Path "tools\Mettre-a-jour.ps1" -Destination (Join-Path $DistFolder "tools")
}
if (Test-Path "res") {
    Copy-Item -Path "res" -Destination $DistFolder -Recurse -Force
}

Write-Host "[3/4] Compression ZIP..." -ForegroundColor Cyan
$ZipPath = Join-Path $OutputDir "NovyDeck-$Version-Windows.zip"
if (Test-Path $ZipPath) {
    Remove-Item -Path $ZipPath -Force
}
Compress-Archive -Path "$DistFolder\*" -DestinationPath $ZipPath -Force

Write-Host "[4/4] Calcul de l'empreinte SHA-256..." -ForegroundColor Cyan
$Hash = (Get-FileHash -Path $ZipPath -Algorithm SHA256).Hash

# Sauvegarde du checksum à côté de l'archive
$HashFile = "$ZipPath.sha256"
"$Hash  NovyDeck-$Version-Windows.zip" | Set-Content -Path $HashFile

Write-Host "Archive prête : $ZipPath" -ForegroundColor Green
Write-Host "SHA-256 : $Hash" -ForegroundColor Green
Write-Host "Fichier SHA-256 enregistré : $HashFile" -ForegroundColor Green
