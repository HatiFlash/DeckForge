# NovyDeck 0.8.10 — transparence du logo corrigée

## Correctif

Le logo de la 0.8.9 avait été placé sur un carré noir opaque lors de la création des déclinaisons. Ce fond n'appartenait pas à l'icône demandée.

- Nouveau PNG transparent fourni par Hatim conservé à l'identique comme source.
- Déclinaisons PNG et ICO Windows régénérées sur un canevas entièrement transparent, en conservant le canal alpha et les bords antialiasés.
- Icônes de l'application et de l'installateur, logo du README, captures et ressources Remotion remplacés.
- Vidéo réexportée pour la 0.8.10 ; suppression du cadre rectangulaire de la signature finale.
- Générateur reproductible `scripts/media/generate-branding.py` et test Rust de non-régression de transparence ajoutés.

## Validation

Compilation Windows et 383 tests locaux réussis, tests MCP stdio/HTTP passés. Le nouveau test échoue avec les ressources opaques de la 0.8.9 puis passe avec les fichiers corrigés. Transparence contrôlée dans tous les formats ICO et dans l'icône extraite de l'exécutable Windows ; vidéo finale relue. [Détail des contrôles](https://github.com/HatiFlash/NovyDeck/blob/v0.8.10/docs/TEST-0.8.10.md).

## Compatibilité

Aucun changement du fonctionnement des actions, macros, profils, plugins ou du rendu des touches personnelles. Les identifiants de compatibilité DeckForge restent inchangés. La release 0.8.9 et son tag sont conservés ; cette correction est publiée dans une nouvelle version.

Le MP4 est également disponible séparément dans les fichiers de cette release.
