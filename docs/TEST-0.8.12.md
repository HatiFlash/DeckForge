# Contrôles NovyDeck 0.8.12

Banc Windows 10 (VM de laboratoire), MSVC, `cargo --locked`. Aucun périphérique branché ; le D6 n'a pas été utilisé.

| Contrôle | Résultat |
|---|---|
| `cargo build --locked` / `--release --locked` | OK, sans avertissement |
| Tests (hors plugins, hors décodeur H.264 interactif) | 315 réussis, 0 échec, 10 ignorés |
| Tests de l'exécutable MCP | 9 réussis |
| Tests plugins (`plugin_`, séquentiels) | 70 réussis, 0 échec, 10 ignorés |
| MCP stdio et HTTP (`tests/mcp-*.cjs`) | OK |
| `inspector_recrop_button_stays_visible_under_the_key_preview` | ÉCHEC sur 0.8.11 (« Recadrer » 4,5 px sous le bord à 820 px de haut), RÉUSSI sur 0.8.12 ; vérifie aussi que « Changer l'icône » tient sur une ligne |
| `community_library_waits_for_consent_below_local_plugins` | RÉUSSI : section après les plugins locaux, aucune requête avant consentement |
| `hardware_report::*` (encodage et longueur de l'URL, anonymisation, nom de fichier) | 3 réussis |
| `community_plugins::*`, `https_get::*`, recherche communautaire | 3 + 1 + 1 réussis |
| `release_0812_review` (ignoré par défaut) | Vrai catalogue `novydeck-plugins` lu en HTTPS via WinHTTP (0 entrée) ; rapport sans nom d'utilisateur ; captures de la fenêtre de rapport, de l'inspecteur à 820 px et de l'onglet Plugins relues |
| Icône transparente (coin alpha = 0) | `novydeck.exe` et `NovyDeck-0.8.12-Setup.exe` |
| Mise à jour 0.8.11 → 0.8.12 (installateur 0.8.11 officiel, SHA-256 vérifié) | Version 0.8.12 installée ; `config.json`, fichier d'assets et plugin témoins conservés ; alias présent ; désinstallation : exécutable retiré, données conservées |

Captures README régénérées : `docs/images/novydeck-main.png` et `novydeck-media-crop.png` (inspecteur), les autres sont identiques au pixel.
