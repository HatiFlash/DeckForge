# Validation — DeckForge 0.3

**17 octobre 2026**, Windows x64, D6 3142:0060.

- **46 tests Rust réussis** : 43 dans l’app, 3 dans le connecteur.
- Les 37 binds intégrés et les actions personnalisables sont validés par le modèle.
- Migration d’un ancien config.json vérifiée avec conservation des touches, images
  et contenu de la sauvegarde .bak ; aucun fichier personnel n’est utilisé par ces tests.
- Pages, sous-dossiers, retour, navigation invalide et conservation des contenus testés.
- Chaîne appui décodé → moteur → changement de page → configuration USB à envoyer :
  vérifiée avec un canal USB simulé ; les touches attendent la confirmation de la bonne révision.
- Tests des vrais widgets egui, hors écran : dépôt de bind, échange de touches,
  annulation, sauvegarde et bouton d’enregistrement de Ctrl+Maj+S sans exécuter la macro.
- Les 15 touches restent visibles en 1380×900 et 1060×720, en clair et sombre.
  Les aperçus sont des rendus logiciels des widgets, pas de nouvelles captures Windows.
- Import Elgato synthétique : format historique et contrôleurs, pages/dossiers,
  action inconnue, multi-action partielle et chemin ZIP dangereux.
- Import d’un profil Notion 15 touches réel : 27 actions et 29 icônes converties,
  avertissements lus. Référence externe non redistribuée.
- **7 contrôles stdio et 11 HTTP** du connecteur, avec le binaire compilé et un pont de test.
- Compilation optimisée GNU/MinGW des deux exécutables ; GitHub Actions teste Windows MSVC.

L’affichage des 15 numéros et les appuis 01 → 08 → 15 ont été confirmés sur le D6
pendant les premières vérifications, et une connexion USB réelle a encore été
observée au début du travail de la 0.3. Le protocole reste celui du matériel 3142:0060.

Le contrôle Windows a ensuite été arrêté par l’utilisateur avec Échap. La dernière
révision des pages, icônes LCD et de l’enregistreur n’a donc pas été revérifiée par
de nouveaux gestes ou appuis sur le matériel ; le reste a été vérifié hors écran.
Pas de test d’endurance, veille/reprise ou de toutes les applications externes.
Ni compte IA, ni endpoint public, ni installation de plugins Elgato ne sont configurés.
