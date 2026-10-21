# NovyDeck 0.8.14 — validation Windows

- Banc : VM Windows 10 MSVC, projet `C:\work\DeckForge`, sans adaptateur graphique WGPU compatible. Aucune installation sur le PC utilisateur et aucun essai du D6 physique.
- `cargo build --locked` : 0 (un avertissement `download` non utilisé hors test).
- `cargo test --locked --bin novydeck app_update::` : 4 réussis, 1 ignoré (test réseau/installateur du laboratoire).
- `cargo test --locked --bin novydeck update` : 9 réussis, 1 ignoré.
- `cargo test --locked -- --skip plugin_ --skip windows_decoder_imports_h264_without_ffmpeg` : groupes principaux 320 et 9 réussis ; 11 ignorés dans le groupe principal.
- `cargo test --locked plugin_ -- --test-threads=1` : 70 réussis, 10 ignorés (un groupe auxiliaire sans tests filtrés). Les suites sont des passages distincts ; ne pas additionner aveuglément leurs comptes.
- `cargo build --release --locked`, `tools\Build-Installer.ps1 -Version 0.8.14`, `tests/mcp-transport.cjs`, `tests/mcp-http.cjs` : codes 0. Coins transparents des icônes EXE et Setup (`A=0`).
- Test `#[ignore]` ciblé en laboratoire (activé explicitement) : l'API GitHub a fourni v0.8.13 et `app_update::download` a téléchargé le vrai Setup publié de 36 467 803 octets avec contrôle SHA-256 ; `app_update::launch` a démarré l'installation différée du Setup local 0.8.14 depuis un chemin contenant des espaces.
- Mise à jour du Setup 0.8.13 officiel (empreinte `.sha256` contrôlée) vers le Setup local 0.8.14 : `ProductVersion=0.8.14`, configuration témoin intacte (hash identique), marqueurs `assets` et `plugins` conservés. L'installateur Inno exécuté séparément avec `/relaunch=1` journalise l'entrée `[Run]` et le chemin de `novydeck.exe`.
- Limite du banc : après lancement graphique, le processus se termine avec `Wgpu(NoSuitableAdapterFound("request_adapters returned None"))`. Ni la présence durable du processus relancé ni l'expérience UI réelle n'ont pu être validées sur cette VM ; la relance reste à confirmer sur un bureau compatible. L'opération GitHub v0.8.14 est à vérifier séparément après CI et publication.
