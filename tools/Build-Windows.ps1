param(
    [ValidateSet('All','Release','Test')][string]$Action='All',
    [string]$ToolchainRoot,
    [string]$TargetDirectory,
    [string]$PreviewDirectory
)
$ErrorActionPreference='Stop'
$sourceDir=Split-Path $PSScriptRoot -Parent
if ($ToolchainRoot) {
    $runtimeRoot=(Resolve-Path -LiteralPath $ToolchainRoot).Path
    $env:CARGO_HOME=Join-Path $runtimeRoot 'cargo'
    $env:RUSTUP_HOME=Join-Path $runtimeRoot 'rustup'
    $env:PATH=(Join-Path $runtimeRoot 'winlibs/mingw64/bin')+';'+(Join-Path $env:CARGO_HOME 'bin')+';'+$env:PATH
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=Join-Path $runtimeRoot 'winlibs/mingw64/bin/gcc.exe'
    $cargoExe=Join-Path $env:CARGO_HOME 'bin/cargo.exe'
} else {
    $cargoExe=(Get-Command cargo -ErrorAction Stop).Source
}
if ($TargetDirectory) { $targetDir=[IO.Path]::GetFullPath($TargetDirectory) } else { $targetDir=Join-Path $sourceDir 'target' }
$env:CARGO_TARGET_DIR=$targetDir
if ($PreviewDirectory) {
    $env:DECKFORGE_UI_OUTPUT=[IO.Path]::GetFullPath($PreviewDirectory)
    New-Item -ItemType Directory -Path $env:DECKFORGE_UI_OUTPUT -Force | Out-Null
}
$manifest=Join-Path $sourceDir 'Cargo.toml'
& $cargoExe --version
if ($LASTEXITCODE -ne 0) { throw 'Cargo indisponible. Configure Rust ou indique ToolchainRoot.' }
# A release connector is needed by the end-to-end MCP test, even in Test mode.
& $cargoExe build --release --locked --manifest-path $manifest
if ($LASTEXITCODE -ne 0) { throw 'Compilation impossible.' }
if ($Action -eq 'Release') { return }
$env:DECKFORGE_MCP_EXE=Join-Path $targetDir 'release/deckforge-mcp.exe'
& $cargoExe test --locked --manifest-path $manifest
if ($LASTEXITCODE -ne 0) { throw 'Echec des tests Rust.' }
& node (Join-Path $sourceDir 'tests/mcp-transport.cjs')
if ($LASTEXITCODE -ne 0) { throw 'Echec des tests MCP stdio.' }
& node (Join-Path $sourceDir 'tests/mcp-http.cjs')
if ($LASTEXITCODE -ne 0) { throw 'Echec des tests MCP HTTP.' }
Write-Host 'Compilation et verification terminees. Aucune application native lancee.'
