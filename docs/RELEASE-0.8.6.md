# DeckForge 0.8.6 — pavé 5 × 3 mieux intégré

## Châssis et touches
- Le SVG ne contient plus aucune surface, bordure ni ombre de touche : seulement le châssis 3D. Les 15 touches restent interactives et entièrement dessinées par egui ; un contour par touche, reflet discret et ombre intérieure suggérée par le dégradé.
- Un seul système de coordonnées dérivé du rectangle intérieur du SVG place la grille et le châssis : faces 228 × 214 (au lieu de 202 × 190), écarts 20 × 18 (au lieu de 42 × 35). Centrage et mise à l'échelle uniformes, sans déformation ni décalage fixe rajouté.
- Les deux touches d'exemple « Exemple macro » et « Lecture » n'ont plus de fonds saturés violet/vert dans **les nouvelles configurations** : fond neutre identique aux autres touches. Les profils existants et couleurs choisies volontairement ne sont pas modifiés lors de la mise à jour.
- Aucun changement aux actions, profils, macros, plugins ou au moteur des images/GIF/vidéos personnalisés.

## Vérifications et limites
- Banc Windows : 374 tests réussis, 0 échec, 18 ignorés (notamment périphériques/interactifs) ; compilation et installateur 0.8.6 validés localement. Captures examinées en thèmes sombre/clair à 1060 × 720 et 1380 × 900, plus 1920 × 1080 à 125 % de DPI. Images dans `docs/` issues du banc sans matériel D6.
- Ce banc ne vérifie pas le rendu sur le FIFINE D6 physique ni la présence effective de tous les décodeurs vidéo sur le PC de l'utilisateur. La pointe mémoire de la 0.8.4 est restée non reproduite, sans cause établie.
