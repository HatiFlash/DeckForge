# Identité visuelle de NovyDeck

Ce document décrit les règles visuelles **appliquées dans le code** (`src/theme.rs`, `src/deck_chassis.rs`). Il sert de référence pour les nouvelles fenêtres, la documentation et la vidéo. Depuis la 0.10.0, l'interface suit le design system ci-dessous, inspiré de Linear, Raycast, Figma et du Stream Deck, rendu nativement avec egui.

## Principes

- Thème sombre par défaut (bleu nuit profond et graphite), thèmes clairs complets. Les couleurs sont lues dans la palette courante (`theme::palette`), jamais codées en dur dans les écrans.
- Un seul accent pour la sélection et l'action principale. Le vert signale ce qui est connecté ou réussi, l'orange un avertissement et le rouge un danger. Une couleur n'est jamais le seul indice : un texte ou une icône l'accompagne.
- Pas d'émojis comme icônes de navigation, pas de lueurs néon ni d'effets coûteux image après image.
- Le contenu des touches (images, GIF, vidéos, couleurs) appartient à l'utilisateur. Le style des icônes de l'interface ne lui est jamais imposé.

## Design system (0.10.0)

**Principe :** calme, précis, matériel. La profondeur vient des niveaux de fond plutôt que de bordures partout ; un seul accent ; le chrome reste discret pour que le deck soit le héros. La disposition ne change pas : navigation à gauche, deck et inspecteur au centre, bibliothèque d'actions à droite.

### Surfaces et textes (`Palette`)

| Jeton | Bleu nuit | Graphite | Clair | Sable | Usage |
|---|---|---|---|---|---|
| `canvas` | `#0B0F17` | `#0C0D10` | `#F5F6F8` | `#F7F4EF` | fond de l'application |
| `sunken` | `#090E16` | `#0A0B0E` | `#EBEDF1` | `#EEE9E1` | champs, zones encastrées, onglets segmentés |
| `surface` | `#111723` | `#121418` | `#FFFFFF` | `#FFFDF9` | panneaux (navigation, bibliothèque, inspecteur), scène du deck |
| `raised` | `#18202E` | `#191C21` | `#F0F2F5` | `#F2EDE5` | cartes, boutons secondaires, éléments survolés |
| `overlay` | `#1E2737` | `#20242A` | `#FFFFFF` | `#FFFDF9` | menus, popovers |
| `hairline` | `#222B3A` | `#23262D` | `#E4E7EC` | `#E3DCD2` | séparateurs 1 px |
| `border_strong` | `#455266` | `#42464E` | `#C0C7D0` | `#C6BAAC` | contours marqués (châssis, focus hors accent) |
| texte primaire | `#EFF3F9` | `#F3F4F6` | `#0E1116` | `#211D18` | titres et texte courant |
| texte secondaire | `#C7D1DF` | `#CBCFD6` | `#434A55` | `#4D443A` | descriptions |
| texte tertiaire | `#A9B4C5` | `#AEB3BB` | `#565D67` | `#63584C` | légendes, intitulés de section |

`accent` vient du réglage d'accent (bleu par défaut `#3478F6`) ; `accent_hover`, `accent_soft` (fond de sélection, ~14 %), `accent_ring` et `on_accent` en sont dérivés. En clair, l'émeraude et l'orange sont assombris pour garder un contraste suffisant. Les couleurs sémantiques (succès, avertissement, danger) ont chacune une variante `*_soft` pour les fonds de pastilles. Le test `theme` vérifie les contrastes WCAG (texte ≥ 4,5:1, composants ≥ 3:1) pour les 4 thèmes × 5 accents.

### Typographie

Segoe UI pour le texte, **Segoe UI Semibold** (`theme::strong`, famille `strong`, repli sur Segoe UI si absente) pour les titres et les libellés actifs. Échelle : 11 (légende, intitulés de section en majuscules espacées via `theme::overline`), 12 (secondaire), 13 (corps dense et boutons), 15 (sous-titres de panneau), 20 (titre du profil).

### Espacements, rayons, ombres

Grille de 4 (`SPACE` = 4 · 8 · 12 · 16 · 24 · 32). Rayons : 6 contrôles (`RADIUS_SM`), 10 cartes et lignes (`RADIUS_MD`), 14 panneaux, fenêtres et scène (`RADIUS_LG`). Ombres douces uniquement sur fenêtres, menus et popovers, et ombre portée en couches sous le châssis du deck (pas de halo coloré).

### Composants (`src/theme.rs`)

| Composant | Rendu |
|---|---|
| `primary` | accent plein, texte `on_accent` ; une seule action principale par zone (Simuler, Utiliser sur la touche) |
| `ghost_button` / `secondary_button` | fond `raised` discret, **aucun contour au repos**, plus clair au survol, `accent_soft` à l'appui |
| `icon_button` | carré compact avec icône IconPark, infobulle et nom accessible (Annuler, Rétablir…) |
| `segmented` | pilules sur fond `sunken`, élément actif en `raised` (pagination, Appui court / long, onglets de la bibliothèque) |
| `underline_tabs` | onglets texte + icône avec indicateur accent de 2 px (Action / Apparence / Avancé) |
| `list_row` | ligne sans cadre, fond au survol, sélection `accent_soft` + barre d'accent de 2 px à gauche |
| `search_field` | champ encastré avec loupe et texte indicatif tertiaire |
| `overline`, `pill`, `status_badge`, `card` | intitulés de section, pastilles d'état sans contour, cartes `raised` |

### Deck

Châssis graphite avec contour `border_strong`, reflet haut discret et ombre portée douce, posé sur une scène `surface` arrondie. Les faces ont des teintes graphite fixes dans tous les thèmes (elles sont sur le châssis), un contour presque invisible, un éclaircissement au survol (~100 ms) et un anneau d'accent de 2 px sur la touche sélectionnée ; le vert signale toujours un appui. Les images envoyées au périphérique ne changent pas.

### Mouvement

Transitions de survol et d'appui de 100 à 120 ms (`animate_bool_with_time`), uniquement sur l'opacité ou la couleur : aucune cible de clic ne se déplace. Elles sont coupées par **Réduire les animations**.

## Jetons historiques

| Jeton | Valeur | Usage |
|---|---|---|
| `BLUE` | `#3478F6` | accent par défaut, sélection, action principale (cinq accents au choix dans Réglages) |
| `GREEN` | `#22C58B` | connecté, réussi, touche pressée |
| `WARNING` | `#F5AD42` | avertissement, plugin en erreur |
| `DANGER` | `#F25F70` | action destructrice |
| `SPACE` | 4 · 8 · 12 · 16 · 24 · 32 | espacements ; `item_spacing` = 8 |
| `RADIUS_SM` | 6 | boutons, champs |
| `RADIUS_MD` | 10 | menus, cartes, lignes de liste |
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

- Les captures `docs/images/*.png` sont produites par `readme_captures_from_the_demo_profile` et `readme_captures_redesign_in_both_themes_and_languages` (`src/ui_tests.rs`). Ce test exécute le vrai code d'interface hors écran, à **2560 × 1600 pixels** (1600 × 1000 points à 160 %), avec le profil « NovyDeck Demo / README ». Il ne fait aucun agrandissement après coup, et le filtrage des textures est bilinéaire comme dans le rendu GPU. Le contenu des touches reste l'image réelle de 126 × 126 pixels envoyée au D6.
- La vidéo publiée `docs/media/novydeck-demo.mp4` est un film de présentation de 40 s en 1920 × 1080 à 60 i/s, avec son, réalisé hors du dépôt avec Remotion et three.js : pavé D6 en 3D, captures réelles de l'interface et musique synthétisée. Le pipeline `scripts/media/remotion/` du dépôt reste disponible pour la présentation précédente ; il ne produit pas ce film publié.
