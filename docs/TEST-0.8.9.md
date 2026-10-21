# Validation NovyDeck 0.8.9 — 21 octobre 2026

## Application Windows (VM de banc, sans D6)

- `cargo test --locked --no-run` : succès, binaires de tests NovyDeck et connecteur MCP compilés.
- `cargo build --release --locked` : succès, métadonnées Windows `ProductName=NovyDeck`, `ProductVersion=0.8.9`.
- Suite Rust générale (hors plugins et décodeur Windows nécessitant une session graphique), puis plugins séquentiels : **382 tests réussis, 0 échec, 19 ignorés** au total, connecteur inclus.
- Tests Node MCP stdio et HTTP : succès. Découverte des 11 outils, authentification, erreurs de protocole, protection des doublons, refus non-loopback et limitation des clients lents vérifiés.
- Garde de compatibilité du répertoire de données : le test échoue avec la condition inversée, puis réussit avec le code final restauré. Après restauration par copie, le mtime a été actualisé pour éviter la réutilisation par Cargo du binaire mutant.
- Quatre captures réelles 2560 × 1600 et écrans des 15 touches produits par les tests existants ; captures relues.

## Installateur (VM uniquement)

- Construction Inno Setup 0.8.9 : succès.
- Ancien installateur DeckForge 0.8.8 téléchargé depuis sa release, SHA-256 vérifié.
- Installation 0.8.8 en répertoire de banc ; ajout de cinq fichiers témoins (`config.json`, préférences, variables, média, plugin).
- Mise à jour 0.8.9 **sans préciser un nouveau répertoire** : emplacement précédent retrouvé, cinq empreintes conservées, versions des binaires et alias 0.8.9 vérifiées, démarrage Windows réorienté vers NovyDeck, ancien raccourci remplacé.
- Désinstallation gérée : cinq fichiers témoins conservés.
- Nouvelle installation 0.8.9 dans un dossier distinct : marque NovyDeck vérifiée, aucune configuration utilisateur créée et aucune instance de l'application lancée.
- Le script de mise à jour manuel passe l'analyse syntaxique PowerShell 5. Il ne lance plus automatiquement une nouvelle instance de l'application.

Ces données témoins ne sont pas les profils personnels de Hatim. Aucun test matériel D6 n'a été lancé sur son PC.

## Présentation Remotion (portable Windows Iris Xe)

- Dépendances Remotion 4.0.537 uniformes, installation locale isolée, lockfile issu de l'installation réelle.
- Ressources présentes et `tsc --noEmit` : succès.
- Export Remotion : succès, 1290 images, 1920 × 1080 à 30 i/s, H.264 + AAC, durée conteneur 43,008 s.
- Fichier MP4 : 21 500 333 octets ; SHA-256 `b8788443f48a1ec2d6707c0f8495d64cae969623a3e09158c7b4e907ba41c160`.
- Planche extraite du MP4 final relue ; nouvelle identité visible, pas d'ancienne capture insérée. Bande-son procédurale reconstruite pour les nouveaux chapitres.

## Contrôle de format

`git diff --check` est vert. `cargo fmt --all -- --check` échoue sur le formatage global ; le même contrôle échoue aussi sur la source précédente `e897c1a` (première différence dans `build.rs:20`). Ce renommage ne reformate pas toute la base historique.

## Limites

Les tests de banc et les captures hors écran ne prouvent pas le rendu ni les interactions du D6 physique. Les anciens noms techniques strictement nécessaires à la compatibilité (mutex, clés MCP, chemins et alias) restent intentionnels. Les notes et captures d'anciennes versions sont historiques.
