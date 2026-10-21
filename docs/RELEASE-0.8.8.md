# DeckForge 0.8.8 — coins des touches, profil de démonstration et identité visuelle

## Rendu des touches
- Fonds, images, GIF et images de vidéo sont masqués à l'arrondi de l'écran de chaque touche, dans la grille comme dans l'aperçu de l'inspecteur. Avant, un rectangle à angles droits dépassait sous la bordure arrondie.
- Le masque passe par un maillage texturé aux coins arrondis, avec une couronne d'un pixel physique pour lisser le bord sans halo. Rien n'est recalculé image après image, et le rayon est concentrique à celui de la face (`deck_chassis::lcd_radius`).
- L'image envoyée au D6 n'est pas modifiée : son écran physique reste carré.
- Test de rendu `key_media_is_masked_by_the_rounded_lcd_corners` : fond plein, quatre coins contrôlés à 100, 150 et 200 %.

## Profil de démonstration
- **Profils → ••• → Profil de démonstration (15 touches)** ajoute « DeckForge Demo / README », ou le réinitialise s'il existe déjà. Les autres profils ne sont pas modifiés ; un test le vérifie.
- Les 15 touches utilisent des actions existantes : macro copier-coller, médias, Bloc-notes, scène OBS, dossier, photo, animation, raccourci, signature, notification, micro et page suivante.
- Les deux médias (un paysage et un égaliseur animé) sont dessinés par l'application dans `assets/demo` et clairement nommés comme démonstration. Le profil ne contient ni script ni requête réseau.

## Interface et documentation
- Jetons de design centralisés dans `theme.rs` (espacements, rayons) et décrits dans `docs/DESIGN.md`.
- Propriétés d'un raccourci clavier : le bouton « Enregistrer… » n'est plus coupé quand le panneau est étroit.
- Nouveau README avec une capture principale, des points clés, une compatibilité explicite (disponible ou expérimental), l'installation, une galerie et la documentation.
- Captures réelles à 2560 × 1600 : le vrai code d'interface est rendu hors écran à 160 %, avec un filtrage bilinéaire, sans agrandissement.
- Vidéo de présentation la vidéo archivée dans le [tag v0.8.8](https://github.com/HatiFlash/NovyDeck/tree/v0.8.8/docs/media) : 42 s en 1920 × 1080, H.264 + AAC. Pavé D6 en 3D (three.js, éclairage PBR, bloom), caméra qui traverse les vraies captures de l'application, typographie animée et bande-son procédurale à −14 LUFS. Les 15 écrans de touches sont rendus par le moteur `key_render` de DeckForge. Sources reproductibles dans `scripts/media/promo/` (mise à jour publiée sur `main` après le tag).

## Vérifications et limites
- Vérifié sur la VM Windows : tests automatiques, compilation release et installateur. Captures relues à 100 %.
- Le contenu des touches reste l'image réelle du D6 (126 × 126 pixels) : dans les captures, il est agrandi et donc moins net que le texte de l'interface.
- Les captures et la vidéo ont été produites sans FIFINE D6 connecté. Le profil de démonstration n'a pas été essayé sur le vrai périphérique.
- Les icônes de l'interface restent IconPark : elles n'ont pas été remplacées par Tabler Icons (voir `docs/DESIGN.md`).
