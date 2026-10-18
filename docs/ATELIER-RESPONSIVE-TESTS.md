# Validation responsive atelier — 18 octobre 2026

Correction dans le dépôt, non publiée et non installée dans le dossier habituel.

- Bibliothèque escamotable en mode compact ; propriétés/chat utilisent une place réservée et ne sont pas affichés ensemble.
- En-tête atelier compact, commandes secondaires dans Réglages, arrêt et activation accessibles.
- Saisie assistant fixe au-dessus du contenu défilable ; pied de page tronqué avec infobulle.
- Minimum fenêtre : 1000 × 700 unités logiques ; aucune borne maximale ajoutée.

## Vérifications réelles
Application compilée lancée seule dans Desktop\DeckForge-0.7-test-atelier, avec copie de configuration. Aucune macro réelle exécutée.
Captures du contenu : 1000×700, 1344×712 (fenêtre extérieure 1366×768), 1898×1024 (extérieur 1920×1080). Chat, saisie « Attends 2 s », proposition, défilement, fermeture, sélection et propriétés vérifiés. Pas de superposition entre les panneaux réservés et les cartes sur ces parcours.
Redimensionnement manuel à la souris : contenu minimum 1500×1050 avec le DPI Windows actif (150 %). SetWindowPos peut forcer une taille sous le minimum : ce n'est pas une preuve de redimensionnement manuel autorisé.
Application habituelle Desktop\DeckForge-0.5-diagnostic restaurée après les essais, processus répondant. Aucun binaire habituel remplacé.

## Automatisation et limites
Build complet vert : 266 tests applicatifs, 1 ignoré, 9 MCP. Test egui tailles/DPI ajouté. Cargo check et rustfmt ciblé vérifiés séparément. Formatage global historique non repris.
Les changements de DPI 100/125/150 % sont couverts par le harness, pas par des changements globaux des réglages Windows. Le mode flottant des très grandes largeurs logiques n'a pas été testé dans une vraie fenêtre dans cette passe. Aucun essai d'exécution réelle, envoi IA ou publication.
