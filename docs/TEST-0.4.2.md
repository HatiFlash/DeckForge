# Essais manuels — rendu 0.4.2

La version en cours reste ouverte : ce correctif n’a pas été installé pendant le travail.

## Installer le correctif d’affichage depuis la version 0.4.1

1. Quitte uniquement DeckForge avec **Réglages → Quitter DeckForge**.
2. Lance **Mettre-a-jour-affichage.cmd** depuis le dossier de mise à jour. Il remplace le programme après sauvegarde et conserve le connecteur MCP existant, compatible avec ce correctif. Codex peut rester ouvert. Les profils, préférences, icônes et accès locaux sont conservés.
3. Rouvre DeckForge depuis son dossier habituel. Réactive **Assistant IA** si tu utilises le MCP : ce réglage n’est pas conservé au redémarrage dans cette version.

Pour une mise à jour complète depuis une version plus ancienne, quitte aussi le connecteur/client MCP puis utilise **Mettre-a-jour.cmd**. Une archive téléchargée doit être extraite ; l’outil te demandera le dossier de l’application existante.

## Comparer l’aperçu au D6

- Vérifie **D6 connecté** et le même profil/la même page. En mode déconnecté, le D6 peut encore afficher une configuration antérieure.
- Compare Lecture, Volume +/− et Muet : mêmes contours blancs, fond et titre dans l’aperçu et sur le D6.
- Vérifie une image personnalisée : même cadrage et même titre. Une image ne s’affiche plus en miniature à l’intérieur de la touche de l’éditeur.
- Bascule le bouton audio : l’aperçu et l’écran de la touche doivent utiliser la même image pour chaque état.
- Vérifie **Page précédente**, **Page suivante** et **Exemple macro** : les retours à la ligne doivent respecter les mots, avec une ellipse si le titre est trop long.

Le cadre en relief, le contour de sélection, les numéros et le signe + des touches libres sont des aides de l’éditeur. Ils ne font pas partie de l’image LCD. La luminosité et la restitution physique du panneau ne sont pas simulées.
