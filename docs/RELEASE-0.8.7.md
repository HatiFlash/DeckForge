# DeckForge 0.8.7 — plugins, atelier et conditions d'application

## Interface
- Ligne plugin de l'inspecteur : boutons « Réglages du plugin » et « Retirer le plugin » placés à droite et toujours visibles ; identifiant raccourci et tronqué à gauche, nom complet et état au survol.
- Atelier de blocs en mode Simple : un clic sur un bloc (y compris imbriqué dans ALORS/SINON) affiche ses propriétés. En fenêtre compacte, la bibliothèque de blocs ou l'assistant qui occupait la colonne est refermé.

## Assistant local
- Condition écrite après l'action comprise par les règles : « ouvre le navigateur Brave s'il n'est pas ouvert et s'il est déjà ouvert ouvre un nouvel onglet » → Si brave.exe tourne : activer + Ctrl+T, sinon : lancer brave.exe. Consigne équivalente ajoutée à `res/ia/ia-locale.json` pour l'IA locale.
- Raccourcis de navigateur reconnus dans les conditions : rouvrir l'onglet fermé (Ctrl+Maj+T), onglet suivant/précédent (Ctrl+Tab / Ctrl+Maj+Tab), plein écran (F11).

## Exécution
- « Activer la fenêtre » : si Windows refuse le premier plan (touche pressée depuis le D6), DeckForge réessaie en partageant l'entrée avec la fenêtre active, puis laisse 120 ms à l'application avant le raccourci suivant. Un refus persistant reste signalé comme erreur.
- Backport du correctif officiel egui #7928 dans `vendor/egui-wgpu` (egui/eframe restent en 0.31.1). Banc : sortie forcée après préparation des buffers, mémoire WGPU 3,77 Go en 9 s sans correctif, stable à 0,92 Mo avec. La minimisation ordinaire n'a pas reproduit la hausse sur la VM ; la cause des 35 Go signalés reste non démontrée.

## Limites
- Vérifié sur la VM Windows sans FIFINE D6 ni Brave réel : 378 tests réussis, 0 échec, 18 ignorés, compilation release et installateur ; tests automatiques et captures du banc. L'effet de Ctrl+T dans Brave et l'activation depuis le vrai D6 restent à confirmer sur le PC.
