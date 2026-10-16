# Validation — DeckForge 0.2

**16 octobre 2026**, Windows x64, D6 `3142:0060`.

- **28 tests Rust réussis** : 25 dans l’app et 3 dans le connecteur.
- **7 vérifications stdio réussies** : initialisation/négociation, découverte des
  outils, refus quand désactivé, aller-retour authentifié, UTF-8 et doublons,
  méthodes inconnues, fermeture sans sortie parasite.
- **11 vérifications HTTP réussies** : Bearer, Origin, sessions, découverte,
  notifications 202, aller-retour/doublons, GET 405, version invalide, JSON invalide,
  suppression de session et refus d’une écoute hors localhost.
- Compilation optimisée des deux exécutables sous GNU/MinGW.
- Rendu normal et compact vérifié visuellement à l’échelle Windows du PC.
- Connexion réelle : `connected: true`, état des touches `Execute`.

Le test de routage rejoue les captures matérielles **appui 01, relâchement 01,
appui 08, relâchement 08, appui 15**. Seules trois exécutions atteignent le moteur,
avec des délais sans effet sur les autres applications.

Les tests IA vérifient qu’une proposition reste un brouillon avant validation,
qu’une modification la rend obsolète, qu’une macro invalide est refusée et qu’une
demande d’exécution peut être rejetée sans effet.

Les tests de transport utilisent un pont local de test. Les tests Rust du pont
couvrent authentification, activation et aller-retour. Ils ne signifient pas
qu’un compte ChatGPT ou Claude a été configuré.

L’utilisateur a confirmé l’affichage des 15 numéros et les appuis physiques
01 → 08 → 15. Le descripteur et la synchronisation ont été revérifiés en 0.2.

Pas de test d’endurance sur plusieurs heures ni de veille/reprise dans cette
session. Toutes les actions dans les applications externes n’ont pas été exercées.
Publication d’un endpoint distant, OAuth et installation dans un compte IA sont
hors du périmètre de ces tests. Le workflow GitHub ajoute les tests Windows MSVC ;
son résultat est consultable dans Actions et n’est pas anticipé par ce rapport local.
