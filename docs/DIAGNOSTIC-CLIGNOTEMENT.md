# Diagnostic du clignotement (version de test 0.5)

Sur le PC fixe, laisser l’application actuelle ouverte jusqu’à la bascule convenue : ne pas lancer deux instances DeckForge.

1. Dans Réglages → Réglages du D6, cliquer « Démarrer le diagnostic (90 s max) ». Fermer la fenêtre Réglages pour libérer la zone de survol.
2. Déplacer plusieurs fois la souris de la prévisualisation D6 (haut) à l’inspecteur (bas) ; noter mentalement quand la fenêtre clignote. Ne pas déclencher de macro.
3. Rouvrir Réglages → Réglages du D6 et cliquer « Arrêter le diagnostic ». Cliquer « Copier le chemin du diagnostic », puis envoyer ce chemin au coordinateur. En cas d'oubli, la capture cesse automatiquement après 90 s / 6 Mo.

Le fichier JSONL reste local sous `diagnostics` à côté de l'application, ou sous `%LOCALAPPDATA%/DeckForge/diagnostics` si le premier emplacement est inaccessible. Il contient des mesures numériques d'interface et des catégories fixes, sans noms d'assets, macros, texte utilisateur ou messages de plugins. `frame_interval_ms` est l’intervalle entre updates observés, pas un temps GPU mesuré ; `update_ms` mesure uniquement l'appel App::update, pas le rendu complet eframe. `cache_misses` et `texture_recreates` sont des compteurs de previews UI, pas les textures D6. `repaint_animation_ms` couvre les demandes de repaint du cache animé, pas tous les repaint d'eframe. Aucun correctif du clignotement n'est revendiqué.
