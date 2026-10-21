# Contrôles NovyDeck 0.8.13

Banc Windows 10 (VM de laboratoire), MSVC, `cargo --locked`. Aucun périphérique branché ; le D6 n'a pas été utilisé et aucun périphérique audio n'a été basculé.

| Contrôle | Résultat |
|---|---|
| `audio_switch_can_be_dropped_before_choosing_its_devices` | ÉCHEC sur 0.8.12 (« Affectation refusée : Choisis deux périphériques audio différents »), RÉUSSI sur 0.8.13 ; vérifie aussi le refus du même périphérique choisi deux fois |
| Tests (hors plugins, hors décodeur H.264 interactif) | 316 réussis, 0 échec, 10 ignorés |
| Tests de l'exécutable MCP | 9 réussis |
| Tests plugins (`plugin_`, séquentiels) | 70 réussis, 0 échec, 10 ignorés |
| MCP stdio et HTTP (`tests/mcp-*.cjs`) | OK |
| Icône transparente (coin alpha = 0) | `novydeck.exe` et `NovyDeck-0.8.13-Setup.exe` |
| Mise à jour 0.8.12 → 0.8.13 (installateur 0.8.12 officiel, SHA-256 vérifié) | Version 0.8.13 installée ; `config.json`, fichier d'assets et plugin témoins conservés ; alias présent ; désinstallation : exécutable retiré, données conservées |

Aucune modification visuelle : les captures du README sont inchangées.
