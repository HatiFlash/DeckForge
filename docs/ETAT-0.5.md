# DeckForge 0.5 (développement) — état au 17 octobre 2026

Point de reprise après la session Claude du 17/10/2026. Version de travail : branche `claude/0.5`, commit de fusion `45d73e1` (+ ce document). Construite et vérifiée par `tools/Build-Windows.ps1 -Action All` : 162 tests application + 6 tests MCP, contrôles MCP stdio/HTTP, aucun avertissement de compilation, Clippy ≤ 30. **Pas encore validée sur le D6 dans sa forme fusionnée.**

Exécutables prêts sur le PC fixe : `Bureau\DeckForge-0.5-fusion\DeckForge.exe` (+ `deckforge-mcp.exe`, config, assets, `plugins\`). Version d'essai plugins seule : `Bureau\DeckForge-plugins-test`.

## Nouveautés 0.5

| Domaine | Contenu | Vérifié |
|---|---|---|
| Plugins Stream Dock / FIFINE (`plugins.rs`, `plugin_import.rs`, `plugin_host.rs`, `plugin_runtime.rs`, `plugin_http.rs`, `ui_plugin_store.rs`) | Magasin en cartes (Disponibles / Installés, Importer tout) ; hôte WebSocket compatible SDK Elgato/Stream Dock ; démarrage à la demande des plugins visibles ; willAppear/willDisappear ; keyDown/keyUp ; setTitle/setImage/setState/showOk/showAlert → face runtime envoyée au D6 (`Command::KeyImage`) ; icône par défaut du manifeste ; réglages (settings, global settings → `plugin-settings.json`) ; redémarrage borné (3 en 5 min) puis état erreur ; plugins HTML dans Edge headless ; Property Inspector dans une fenêtre Edge `--app`, habillé aux couleurs de DeckForge | Tests + **essai réel** : VolumeController affiche icône et « 50 % » sur le D6 ; Windows Gizmos démarre ; PI s'ouvre et se connecte |
| OBS (`obs.rs`, `obs_monitor.rs`) | WebSocket v5 + authentification ; scène, enregistrement, stream, caméra virtuelle, replay, mute, visibilité de source ; connexion persistante avec reconnexion ; état REC/LIVE/CAM sur la touche ; listes de scènes/sources dans l'atelier | Faux serveur local uniquement |
| Actions natives | Soundboard WAV/MP3 (MCI, 8 sons max, stop) ; souris (clic, déplacement, molette) ; fermer la fenêtre active (protège DeckForge) ; luminosité/veille du D6 ; UDP ; HTTP sans TLS (vMix, Home Assistant local) ; touche à deux états `Toggle` avec badge 1/2 ; assistant raccourcis Discord ; schémas MCP de ces actions | Tests uniquement |

Correctifs trouvés pendant l'essai réel : UUID de plugin absent du manifeste → déduit du dossier ; `CodePathWin` prioritaire sur `CodePath` ; chemins HTTP `%20` (dossier « property inspector ») ; le PI doit être connecté via `connectElgatoStreamDeckSocket` (SDK MiraBox/Stream Dock) et recevoir `didReceiveSettings` à l'ouverture.

## Problèmes ouverts (priorité)

1. **PI VolumeController : liste « Périphérique » vide.** La page se connecte et se traduit, mais le plugin ne lui renvoie pas la liste des périphériques. Pistes : (a) journaliser tous les messages PI ↔ plugin (`sendToPlugin`, `sendToPropertyInspector`) dans le Journal pour voir qui ne répond pas ; (b) vérifier le `context` du `sendToPropertyInspector` émis par le plugin (le routage actuel exige `context == contexte de la touche`) ; (c) le plugin peut attendre `propertyInspectorDidAppear` avec un payload/`device` précis, ou l'info `devices` d'inscription du plugin. Le titre de fenêtre « Input Device Control » sur une touche liée à `outputDeviceActionButton` est à vérifier aussi (mauvais `PropertyInspectorPath` choisi ?).
2. **Réglages de plugin dans une fenêtre séparée.** Choix technique : la page de réglages d'un plugin est du HTML/JavaScript qui dialogue en WebSocket ; l'interface DeckForge (egui) ne sait pas afficher du HTML. Options pour l'intégrer : (a) WebView2 incrusté dans la zone inspecteur (crate `wry`/`webview2-com`, fenêtre enfant positionnée sur le panneau) — rendu identique à Stream Dock, effort moyen ; (b) formulaires natifs egui pour les plugins les plus utilisés (VolumeController, Discord, OBS) — plus beau, plugin par plugin ; (c) a minima, fenêtre Edge accrochée/positionnée sur l'inspecteur et fermée avec la sélection.
3. Windows Gizmos lancé en deux processus : vérifier qu'un plugin n'est démarré qu'une fois.
4. Test `plugin_host::node_fixture_exchanges_key_messages` encore instable sous charge (passe en relance).
5. Essai réel de la 0.5 fusionnée : plugins, OBS réel, Soundboard, Toggle, luminosité/veille, Discord MiraBox (autorisation Discord à la première connexion).

## Branches

| Branche | Rôle |
|---|---|
| `main` | 0.4.3 publiée (Codex) puis avance rapide vers la 0.5 de développement |
| `claude/0.5` | Intégration 0.5 (fusion de `claude/native` et `claude/plugins`) |
| `claude/native` | Actions natives + OBS |
| `claude/plugins` | Système de plugins |
