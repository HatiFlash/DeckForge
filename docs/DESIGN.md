# Identité visuelle de NovyDeck

Ce document décrit les règles visuelles **déjà appliquées dans le code** (`src/theme.rs`). Il sert de référence pour les nouvelles fenêtres, la documentation et la vidéo. Il ne décrit pas une refonte.

## Principes

- Thème sombre par défaut (bleu nuit et gris ardoise), thème clair complet. Les couleurs sont lues dans la palette courante (`theme::palette`), jamais codées en dur dans les écrans.
- Un seul accent pour la sélection et l'action principale. Le vert signale ce qui est connecté ou réussi, l'orange un avertissement et le rouge un danger. Une couleur n'est jamais le seul indice : un texte ou une icône l'accompagne.
- Pas d'émojis comme icônes de navigation, pas de lueurs néon ni d'effets coûteux image après image.
- Le contenu des touches (images, GIF, vidéos, couleurs) appartient à l'utilisateur. Le style des icônes de l'interface ne lui est jamais imposé.

## Jetons

| Jeton | Valeur | Usage |
|---|---|---|
| `BLUE` | `#3478F6` | accent par défaut, sélection, action principale (cinq accents au choix dans Réglages) |
| `GREEN` | `#22C58B` | connecté, réussi, touche pressée |
| `WARNING` | `#F5AD42` | avertissement, plugin en erreur |
| `DANGER` | `#F25F70` | action destructrice |
| `SPACE` | 4 · 8 · 12 · 16 · 24 · 32 | espacements ; `item_spacing` = 8 |
| `RADIUS_SM` | 8 | boutons, champs |
| `RADIUS_MD` | 12 | menus, cartes, panneaux non interactifs |
| `RADIUS_LG` | 14 | fenêtres |
| `KEY_LCD_RADIUS` | 6 | écran d'une touche hors châssis SVG |

Typographie : texte courant à 14 pt, petite légende à 11–12 pt, titres de section à 18–21 pt. Les tailles sont en points ; egui les adapte au facteur d'échelle de Windows (100 à 200 %).

## Touches

- **Géométrie :** la face d'une touche et son écran partagent le même repère (`deck_chassis::face`, `deck_chassis::lcd`). L'écran est arrondi selon un rayon **concentrique** à celui de la face (`deck_chassis::lcd_radius`).
- **Masquage :** fonds, images, GIF et images de vidéo sont dessinés par `theme::rounded_image`. Cette fonction utilise un maillage texturé aux coins arrondis, avec une couronne d'un pixel physique qui passe de l'opaque au transparent. Le `clip_rect` d'egui étant rectangulaire, il ne suffirait pas.
- **Performances :** aucun masque n'est recalculé pixel par pixel ; seule la géométrie change avec la taille.
- **Fidélité au D6 :** l'image envoyée au D6 n'est pas modifiée. L'écran physique reste carré, l'arrondi ne concerne que l'affichage dans l'application.
- **Test :** `key_media_is_masked_by_the_rounded_lcd_corners` vérifie les quatre coins à 100, 150 et 200 %.

| Avant | Après |
|---|---|
| ![Fond carré sous la face arrondie](images/key-corners-before.png) | ![Fond masqué à l'arrondi de l'écran](images/key-corners-after.png) |

## Icônes

L'interface utilise un sous-ensemble verrouillé d'[IconPark](https://github.com/bytedance/IconPark) (Apache 2.0) : les 66 pictogrammes utilisés sont embarqués dans `res/iconpark`, recolorés par famille d'actions et mis en cache en texture. Ils n'ont pas été remplacés par Tabler Icons. Un tel changement toucherait plus de soixante écrans et icônes de touches par défaut, pour un gain de cohérence nul : la bibliothèque actuelle est déjà homogène, sous licence libre et embarquée localement.

## Transparence du logo (0.8.10)

La source utilisateur RGBA et son alpha sont conservés. Les déclinaisons sont redimensionnées sur un canevas `(0,0,0,0)`, jamais sur un fond noir opaque. Le générateur `scripts/media/generate-branding.py` contrôle les coins transparents des PNG et de chaque taille ICO. Relire les aperçus sur fond blanc, gris et sombre avant publication ; ces fonds de contrôle ne sont pas incorporés aux ressources.

## Captures et vidéo

- Les captures `docs/images/*.png` sont produites par `readme_captures_from_the_demo_profile` (`src/ui_tests.rs`). Ce test exécute le vrai code d'interface hors écran, à **2560 × 1600 pixels** (1600 × 1000 points à 160 %), avec le profil « NovyDeck Demo / README ». Il ne fait aucun agrandissement après coup, et le filtrage des textures est bilinéaire comme dans le rendu GPU. Le contenu des touches reste l'image réelle de 126 × 126 pixels envoyée au D6.
- La vidéo publiée `docs/media/novydeck-demo.mp4` est un film de présentation de 40 s en 1920 × 1080 à 60 i/s, avec son, réalisé hors du dépôt avec Remotion et three.js : pavé D6 en 3D, captures réelles de l'interface et musique synthétisée. Le pipeline `scripts/media/remotion/` du dépôt reste disponible pour la présentation précédente ; il ne produit pas ce film publié.
