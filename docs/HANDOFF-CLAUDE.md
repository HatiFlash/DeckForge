# Reprise de DeckForge — 0.4.3, 17 octobre 2026

DeckForge remplace le logiciel FIFINE pour un AmpliGame D6 sous Windows. L’utilisateur privilégie une interface française simple et soignée, les macros avancées, les pages/dossiers et les automatisations modifiables. L’app fonctionne indépendamment du logiciel FIFINE ; les binaires propriétaires ne sont pas intégrés.

## État actuel

La synchronisation, les appuis physiques, le basculement audio et les blocs ont été confirmés par l’utilisateur. Le MCP direct de Codex a été testé en lecture sur l’application réelle : état, profils et périphériques audio. La version 0.4.2 a unifié le rendu des touches avec les images LCD. La version 0.4.3 ajoute Galerie en grand à gauche, l’import image + à droite et le choix d’un fond noir sans perdre la couleur enregistrée. Les derniers correctifs sont préparés séparément ; vérifier la version réellement installée avant d’intervenir sur le PC.

## Carte des sources

| Module | Responsabilité |
|---|---|
| `main.rs` | Point d’entrée, instance unique Windows, options de lancement |
| `ui.rs` | Éditeur egui, état de l’app, Runner indépendant du dessin de la fenêtre |
| `ui_appearance.rs`, `ui_key_preview.rs` | Contrôles de couleur/icône et cache des aperçus |
| `ui_icons.rs`, `ui_extras.rs` | Galerie, états audio, atelier de blocs, réglages Windows |
| `model.rs`, `store.rs` | Modèle, validation, migrations, sauvegarde atomique avec sauvegarde précédente |
| `device.rs`, `control.rs` | Protocole HID, synchronisation révisée, autorisation d’exécuter les touches |
| `key_render.rs`, `icons.rs` | Image LCD unique, texte, symboles, composition alpha et JPEG |
| `engine.rs`, `automation.rs` | Exécution annulable des macros/blocs, export et scripts AutoHotkey |
| `audio.rs`, `desktop.rs`, `recorder.rs` | Core Audio Windows, arrière-plan/hotkeys globaux, enregistrement clavier |
| `catalog.rs`, `assets.rs`, `elgato.rs` | Binds, packs locaux, import de profils et ressources |
| `assistant.rs`, `mcp_server.rs` | Propositions à valider et pont local authentifié |
| `mcp_protocol.rs`, `bin/deckforge-mcp.rs`, `bin/mcp_http.rs` | Schémas MCP, stdio et HTTP local |
| `ui_tests.rs`, `tests/*.cjs` | Widgets sans fenêtre native, intégration MCP et transports |

## Contrats à conserver

- La configuration sur disque peut contenir des touches incomplètes. La configuration déployée sur le D6 est validée et les appuis n’exécutent que la révision synchronisée.
- Le rendu canonique fabrique le JPEG envoyé au D6. L’éditeur affiche ce JPEG décodé en annulant sa rotation de transport. Ne pas réintroduire une seconde version des symboles ou du texte dans l’aperçu.
- `Key.background` est optionnel à la lecture et vaut `true` pour les données anciennes. `false` donne un fond noir ; `Key.color` reste mémorisé. Les zones alpha des icônes se composent sur ce fond. Le schéma de configuration reste 2. Une ancienne version peut ignorer puis supprimer ce champ en réenregistrant : conserver une sauvegarde avant retour arrière.
- Les modèles de l’IA proposent des changements via MCP ; l’app demande une validation locale avant application. Une validation de configuration ne lance pas une macro. Les demandes d’exécution ont leur propre validation et délai de trois secondes.
- Le moteur continue lorsque la fenêtre n’est pas dessinée. Ne pas déplacer les sauvegardes, USB ou événements métier uniquement dans le code de rendu.

## Vérifier sans gêner l’utilisateur

Utiliser les tests Rust et le rendu logiciel existant. Les configurations de test sont temporaires ; le matériel, l’audio réel et les fenêtres natives ne sont pas nécessaires. L’utilisateur travaille parfois sur le même écran : ne pas ouvrir/relancer l’application ni lancer une macro simplement pour vérifier une mise en page.

Pour compiler sur Windows, voir le README et le script de construction du dossier de reprise. `DECKFORGE_MCP_EXE` pointe sur le connecteur release pour activer le test de bout en bout. Le scénario AutoHotkey installé est conditionnel ; ne pas confondre son retour anticipé avec une vérification d’un interpréteur réel.

## Limites et pistes prioritaires

Les plugins tiers Elgato/FIFINE sont inventoriés, sans moteur d’exécution compatible. Les profils Elgato ne sont importés que pour les actions reconnues. Le magasin est un catalogue local de 40 binds, pas un service de distribution de plugins. Aucun modèle IA local n’est intégré. L’accès ChatGPT distant peut nécessiter une connexion/tunnel adaptée au client ; le HTTP écoute uniquement localhost.

Assistant IA est désactivé à chaque lancement. La restauration complète depuis la zone de notification et le démarrage Windows restent à vérifier manuellement. Le retour visuel audio est défini pour un seul basculement audio dans une macro ; plusieurs basculements ne reçoivent pas un état supposé. L’utilisateur souhaite à terme un accès plus simple au Marketplace, de meilleurs imports et des automatisations accessibles par langage naturel via son IA.

Commencer par lire les tests et demander un exemple concret avant de remplacer le moteur ou de changer les formats. Les petits changements d’interface peuvent rester locaux ; les améliorations doivent garder le comportement du D6 et les données existantes.
