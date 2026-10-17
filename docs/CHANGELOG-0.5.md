# DeckForge 0.5 — fiabilité (lot 1)

- Dialogues de fichiers et dossiers déplacés hors du verrou de l’application ; les résultats reviennent au frame suivant via un canal. La lecture Stream Deck et l’analyse des packs d’icônes sont également faites sur le thread de dialogue. Les sélections d’étapes sont appliquées à la touche/page d’origine, si elle existe encore.
- Sauvegarde `config.json.bak` par fichier temporaire puis remplacement atomique. Un `config.json` invalide peut être récupéré depuis la sauvegarde sans détruire celle-ci ; récupération et erreur de sauvegarde à la fermeture consignées dans `deckforge-error.log`.
- Import Stream Deck : rejet des codes de touche Windows dépassant `u16`. Jetons MCP comparés à durée indépendante du préfixe commun. HTTP MCP limité à huit connexions actives, avec réponse 503 en cas de saturation et délais existants.
- Bouton renommé « Arrêter les macros en cours » : il n’arrête pas les applications déjà ouvertes.

## Vérifications exécutées

- `tools/Build-Windows.ps1 -Action All` : 78 tests de l’application et 5 tests du binaire MCP réussis ; tests Node stdio et HTTP réussis (dont huit clients lents, refus 503 et reprise) ; compilation sans avertissement de build.
- `cargo clippy --locked --all-targets` : réussi ; 31 avertissements dans la cible application-tests (base 33 avant ce lot) et 2 dans le binaire MCP avant les deux simplifications mineures finales.

## Restant sur matériel / sécurité

- Vérifier sur D6 physique les appuis USB et MCP pendant un dialogue Windows prolongé, ainsi que l’import de packs et les chemins d’icônes/étapes. Aucun programme natif ni macro réelle n’a été lancé pendant ces tests.
- La restriction ACL du fichier `mcp-connection.json` à l’utilisateur courant n’est pas encore appliquée ; le jeton reste local mais dépend des permissions du dossier contenant l’exécutable.

## Lot 2 - Interface et édition

- Copier / coller / dupliquer une touche (menu clic droit sur l'aperçu, Ctrl+C / Ctrl+V), y compris entre pages et profils.
- Annuler / Rétablir multi-niveaux (50 étapes, frappes regroupées par champ) : boutons en haut, Ctrl+Z / Ctrl+Y. Retrait d'étape annulable.
- Simuler devient le bouton principal ; Tester dans 3 s est secondaire. Flèches ↑ puis ↓ partout.
- Bouton de thème explicite ; indicateur D6 coloré (connecté / en attente / déconnecté) ; aperçu du deck agrandi.
- Correctif : la Galerie ciblait l'ancienne touche après changement de sélection (Touche 07 au lieu de 08).
- Bouton Retirer l'image intégré à la ligne Galerie (la colonne de l'inspecteur était coupée).
- Seuls des glyphes présents dans les polices egui par défaut sont utilisés (↶ ↷ ✕ s'affichaient en carrés).

Tests exécutés : Build-Windows.ps1 -Action All : 81 tests application, 5 tests connecteur, MCP stdio et HTTP OK ; aperçus régénérés et contrôlés visuellement.
## Lot 3a/3b - Atelier de blocs

- L'atelier s'ouvre dans la zone centrale (plus de fenêtre flottante qui masque le deck) avec Retour au deck ; la bibliothèque de droite est masquée pendant l'édition.
- Palette de blocs par catégories avec recherche ; ajout par clic ; + Insérer ici entre deux blocs.
- Cartes repliables avec résumé lisible (ex. Attendre 250 ms, Mémoriser mode = travail), barre de couleur par catégorie, boutons ↑ ↓ Dupliquer Désactiver Retirer.
- Bloc désactivé : conservé mais ignoré à l'exécution, en simulation, à la validation et à l'export AutoHotkey. Sérialisé comme {"type":"disabled","step":{...}}.
  ATTENTION retour arrière : DeckForge 0.4.3 ne connaît pas ce type et refusera une configuration contenant un bloc désactivé. Réactiver les blocs ou garder config.json.bak avant de revenir en 0.4.3.
- Bloc invalide signalé localement (bordure orange + message).
- Simuler les blocs est le bouton principal.

Tests exécutés : Build-Windows.ps1 -Action All : 86 tests application, 5 tests connecteur, MCP stdio/HTTP OK ; workflow.png contrôlé.