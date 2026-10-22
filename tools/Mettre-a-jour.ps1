param([string]$DestinationDirectory, [switch]$ApplicationOnly)
$ErrorActionPreference='Stop'
$packageDir=$PSScriptRoot
if ([string]::IsNullOrWhiteSpace($packageDir)) { $packageDir=$env:NOVYDECK_UPDATE_DIR }
if ([string]::IsNullOrWhiteSpace($packageDir)) { $packageDir=$env:DECKFORGE_UPDATE_DIR }
try {
    if ([string]::IsNullOrWhiteSpace($packageDir)) { throw 'Dossier de mise à jour introuvable.' }
    $packageDir=[IO.Path]::GetFullPath($packageDir)
    foreach ($name in @('novydeck.exe','novydeck-mcp.exe','sha256-files.json')) {
        if (-not (Test-Path -LiteralPath (Join-Path $packageDir $name) -PathType Leaf)) { throw "Extrais toute l’archive avant de lancer la mise à jour." }
    }
    [string[]]$binaryNames=if ($ApplicationOnly) { @('novydeck.exe') } else { @('novydeck.exe','novydeck-mcp.exe') }
    [string[]]$processNames=if ($ApplicationOnly) { @('DeckForge','novydeck') } else { @('DeckForge','novydeck','deckforge-mcp','novydeck-mcp') }
    $running=Get-Process -Name $processNames -ErrorAction SilentlyContinue
    if ($running) { throw "NovyDeck, DeckForge ou leur connecteur MCP tourne encore. Quitte l’application et déconnecte le client MCP avant la mise à jour." }
    if ([string]::IsNullOrWhiteSpace($DestinationDirectory)) {
        $savedTarget=Join-Path $packageDir 'install-target.txt'
        if (Test-Path -LiteralPath $savedTarget -PathType Leaf) { $DestinationDirectory=(Get-Content -LiteralPath $savedTarget -Raw).Trim() }
    }
    if ([string]::IsNullOrWhiteSpace($DestinationDirectory)) {
        Add-Type -AssemblyName System.Windows.Forms
        $picker=New-Object System.Windows.Forms.FolderBrowserDialog
        $picker.Description='Choisis le dossier contenant NovyDeck ou son ancienne installation DeckForge.'
        if ($picker.ShowDialog() -ne [System.Windows.Forms.DialogResult]::OK) { return }
        $DestinationDirectory=$picker.SelectedPath
        $picker.Dispose()
    }
    $installDir=[IO.Path]::GetFullPath($DestinationDirectory)
    if ($installDir.TrimEnd('\') -eq $packageDir.TrimEnd('\')) { throw "Choisis le dossier de l’ancienne application, pas le dossier de mise à jour." }
    if (-not ((Test-Path -LiteralPath (Join-Path $installDir 'DeckForge.exe') -PathType Leaf) -or (Test-Path -LiteralPath (Join-Path $installDir 'novydeck.exe') -PathType Leaf))) { throw "Ce dossier ne contient ni DeckForge.exe ni novydeck.exe. Aucun fichier n’a été modifié." }
    $hashes=Get-Content -LiteralPath (Join-Path $packageDir 'sha256-files.json') -Raw | ConvertFrom-Json
    foreach ($name in $binaryNames) {
        $sourceFile=Join-Path $packageDir $name
        if ((Get-FileHash -Algorithm SHA256 -LiteralPath $sourceFile).Hash -ne $hashes.$name) { throw "Le fichier $name est incomplet ou différent de la version préparée." }
    }
    # Only application binaries and documentation are updated. No profile, token,
    # preferences file or asset directory is copied, moved or removed.
    foreach ($name in $binaryNames) {
        $target=Join-Path $installDir $name
        $backup=$target+'.avant-0.9.2.bak'
        if ((Test-Path -LiteralPath $target -PathType Leaf) -and -not (Test-Path -LiteralPath $backup)) { Copy-Item -LiteralPath $target -Destination $backup }
    }
    foreach ($name in ($binaryNames+@('README.md','LICENSE','THIRD-PARTY-NOTICES.txt'))) { Copy-Item -LiteralPath (Join-Path $packageDir $name) -Destination (Join-Path $installDir $name) -Force }
    $docs=Join-Path $installDir 'docs'
    New-Item -ItemType Directory -Path $docs -Force | Out-Null
    foreach ($file in Get-ChildItem -LiteralPath (Join-Path $packageDir 'docs') -File) { Copy-Item -LiteralPath $file.FullName -Destination (Join-Path $docs $file.Name) -Force }
    Write-Host 'NovyDeck 0.9.2 est prêt. Tes profils, préférences et icônes sont conservés.' -ForegroundColor Green
    if ($ApplicationOnly) { Write-Host 'Le connecteur DeckForge existant est conservé pour les fonctions déjà disponibles. Certaines fonctions nécessitent une mise à jour complète du connecteur NovyDeck 0.9.2.' }
    Write-Host "Tu peux ouvrir NovyDeck depuis son dossier habituel quand tu veux. Rien n’est lancé automatiquement."
} catch { Write-Host $_.Exception.Message -ForegroundColor Yellow }
Read-Host 'Appuie sur Entrée pour fermer'
