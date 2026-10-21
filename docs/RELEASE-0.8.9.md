# NovyDeck 0.8.9 — nouveau nom, nouveau logo

DeckForge devient **NovyDeck**. Le projet, ses fonctionnalités et l'historique Git sont conservés.

## Identité et distribution

- Nouveau logo fourni par Hatim, décliné sans déformation en PNG et en icône Windows multirésolution.
- Nouveau nom dans l'application, les exécutables `novydeck.exe` / `novydeck-mcp.exe`, le README et les ressources de distribution.
- Dépôt GitHub renommé en `HatiFlash/NovyDeck` ; les anciennes adresses GitHub redirigent vers le même projet.
- Installateur `NovyDeck-0.8.9-Setup.exe`, archive portable et archive source sous la nouvelle marque.

## Compatibilité

- Profils, macros, médias, réglages et plugins existants préservés. Aucune substitution automatique des images personnalisées des touches.
- Les anciens chemins et identifiants nécessaires à la compatibilité DeckForge restent volontairement reconnus.
- L'identité Inno Setup existante est conservée pour les mises à jour des installations DeckForge.
- Rust, egui/eframe 0.31.1 et le correctif WGPU existant sont conservés. Le renommage n'est pas une refonte des actions.

## Présentation

- Captures réelles de l'interface NovyDeck, avec le nouveau logo et le profil de démonstration de 15 touches.
- Nouvelle vidéo Remotion : scènes 3D, animations synchronisées image par image, captures réelles et bande-son.
- Sources de la présentation dans `scripts/media/remotion/`.

## Validation

Compilation Windows release et installateur réussis ; 382 tests Rust et les tests MCP stdio/HTTP sont passés sur le banc. Mise à jour depuis 0.8.8, conservation des données témoins, désinstallation et installation neuve vérifiées. Vidéo Remotion exportée sur le portable et relue. Détail : [TEST-0.8.9.md](https://github.com/HatiFlash/NovyDeck/blob/v0.8.9/docs/TEST-0.8.9.md).

Le MP4 est aussi proposé comme fichier indépendant dans cette release.

## Limites

Les captures sont produites en banc, sans FIFINE D6 connecté. Le rendu matériel sur le pavé de l'utilisateur n'est pas attesté par les tests sans périphérique. Les notes et captures des versions antérieures demeurent des archives historiques DeckForge.
