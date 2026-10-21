# NovyDeck 0.8.11 — marquage du châssis

La mention « DECKFORGE » restée au bas du pavé était un texte peint directement par l'interface, indépendant des fichiers du logo. Elle affiche maintenant **NOVYDECK**.

- Libellé corrigé sans déplacer le marquage ni changer sa police, sa couleur ou la géométrie du châssis.
- Nom accessible du SVG mis à jour.
- Test UI sur le texte réellement dessiné : NOVYDECK doit être présent et DECKFORGE absent dans la vue du pavé.
- Captures de l'application et vidéo de présentation actualisées.
- Le logo transparent et les icônes corrigés en 0.8.10 sont conservés.

## Validation

384 tests locaux réussis, tests MCP stdio/HTTP passés et compilation Windows valide. Le test UI échoue sur l'ancien libellé puis réussit avec le marquage corrigé. Captures et MP4 final relus ; mise à jour de banc 0.8.10 → 0.8.11 avec fichier témoin conservé. [Détail des contrôles](https://github.com/HatiFlash/NovyDeck/blob/v0.8.11/docs/TEST-0.8.11.md).

Les profils, actions, macros, plugins, médias personnels et identifiants historiques de compatibilité ne changent pas. Les anciens noms de dossiers, clés persistantes et outils MCP ne sont pas remplacés aveuglément.

Les releases 0.8.9 et 0.8.10 restent archivées avec leurs tags d'origine.
