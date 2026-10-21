# Icônes IconPark

Les icônes d'interface de NovyDeck proviennent d'**IconPark** (ByteDance) :
https://github.com/bytedance/IconPark — https://iconpark.oceanengine.com/official

- Licence : Apache License 2.0 (texte complet dans `LICENSE`, à côté de ce fichier, et dans la fenêtre « À propos de NovyDeck »).
- Source : paquet officiel `@icon-park/svg` (`packages/svg/src/icons/*.ts`), commit `8dc132da4c85671ba6a5962c87aa2bdafbf158e9`.
- Modifications : seuls les 71 pictogrammes utilisés sont conservés. Leurs 4 couleurs du thème « multi-color » sont
  remplacées par des jetons (`@0@` contour extérieur, `@1@` remplissage extérieur, `@2@` contour intérieur,
  `@3@` remplissage intérieur) que NovyDeck colore selon sa palette (`src/iconset.rs`). Épaisseur de trait 4,
  extrémités et jonctions arrondies, comme les valeurs par défaut d'IconPark. Les tracés ne sont pas modifiés.
