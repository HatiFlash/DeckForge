# Essais manuels — DeckForge 0.4.3

L’application en cours n’a pas été remplacée pendant le travail.

1. Vérifie le grand bouton **Galerie** à gauche et le petit carré **image +** à droite. Le carré ouvre le choix d’un fichier PNG, JPEG ou SVG sur le PC. Le bouton Galerie ouvre les packs locaux.
2. Sélectionne Lecture, choisis une couleur, puis clique sur le **seau barré** à côté de la couleur. Le bouton doit passer sur fond noir dans l’aperçu et sur le D6 après synchronisation. Reclique : la couleur précédente doit revenir.
3. Quitte et relance DeckForge : le choix de fond doit être conservé, y compris dans les binds enregistrés. Les profils anciens gardent leur fond coloré initial.
4. Essaie une icône PNG/SVG avec une zone transparente : celle-ci doit utiliser la couleur choisie ou le noir selon le réglage. Cette option ne supprime pas le fond déjà dessiné dans une photo/image opaque.

Pour appliquer uniquement les changements de l’interface, quitte DeckForge puis lance **Mettre-a-jour-affichage.cmd** et rouvre l’application habituelle. Codex peut rester ouvert. Le connecteur existant peut continuer à lire et proposer les actions déjà disponibles ; il conserve le choix de fond si le champ est absent d’une proposition.

Le nouveau champ MCP `background` exige le connecteur 0.4.3 pour être proposé à l’IA. Pour actualiser aussi ce connecteur, quitte l’application et le client MCP, utilise **Mettre-a-jour.cmd**, puis recharge les serveurs dans le client IA. Réactive Assistant IA dans DeckForge après redémarrage.
