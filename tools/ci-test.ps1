# Tests CI : suite générale en parallèle, puis tests de plugins (processus Node/PowerShell réels)
# un par un, car ils dépendent de délais de démarrage sensibles à la charge du runner.
# Les tests en échec sont publiés en annotations (lisibles sans se connecter à GitHub).
$log = "test-output.log"
Remove-Item $log -ErrorAction SilentlyContinue
$failed = $false
foreach ($set in @(@("--", "--skip", "plugin_"), @("plugin_", "--", "--test-threads=1"))) {
    cargo test --locked @set 2>&1 | Tee-Object -FilePath $log -Append
    if ($LASTEXITCODE -ne 0) { $failed = $true }
}
if ($failed) {
    Select-String -Path $log -Pattern "panicked at|^test .* FAILED|^---- |error:" |
        Select-Object -First 25 | ForEach-Object { "::error title=cargo test::$($_.Line.Trim())" }
    exit 1
}
