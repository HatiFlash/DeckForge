# Validation 0.8.10 — logo transparent

## Ressources

- PNG transparent fourni par Hatim conservé octet pour octet dans `res/branding/novydeck-source.png` et `scripts/media/remotion/public/logo-original.png`.
- Source : RGBA 1536 × 1024, alpha de 0 à 254.
- Logos 1024, 256, 64 et 32 px : alpha de 0 à 255, coins transparents et pixels antialiasés conservés.
- ICO 16, 24, 32, 48, 64, 128 et 256 px : chaque taille vérifiée par Pillow ; les deux fichiers ICO distribués sont identiques.
- Générateur testé : une image entièrement opaque est refusée. Aperçus contrôlés visuellement sur blanc, gris et sombre.

## Application Windows (banc, sans D6)

- Nouveau test `branding_assets_keep_transparency_and_visible_pixels` : échec attendu avec les logos de la 0.8.9 (alpha 255 au lieu de 0), puis réussite avec les ressources corrigées.
- `cargo build --release --locked` : succès, version 0.8.10.
- Icône extraite du véritable exécutable Windows via `System.Drawing.Icon.ExtractAssociatedIcon` : 32 × 32, alpha du coin égal à 0. Même contrôle réussi sur l'installateur.
- Installateur 0.8.10 construit, puis mise à jour de l'installation de banc 0.8.9 sans nouveau dossier : succès, version finale 0.8.10 et empreinte du fichier témoin conservée.
- Quatre captures réelles 2560 × 1600 régénérées ; l'en-tête ne présente plus de carré noir.
- Suite locale : **383 tests réussis, 0 échec, 19 ignorés** ; tests Node MCP stdio et HTTP réussis. Le test Media Foundation nécessitant une session graphique n'a pas été exécuté par SSH.

## Vidéo

- Projet Remotion 0.8.10 : ressources contrôlées, TypeScript valide.
- Film réexporté sur le portable Windows avec les nouveaux logos et captures ; signature finale sans cadre opaque.
- MP4 H.264/AAC, 1920 × 1080, 1290 images à 30 i/s, durée conteneur 43,008 s.
- Taille : 21 518 830 octets ; SHA-256 `b2381e186c2de6205bbfbe2610ec42effb9a14fbb6d51bd47ff0b6926450e4f0`.
- Images extraites du MP4 final relues, notamment introduction et fin.

Les contrôles n'ont pas modifié de profils personnels ni lancé d'action sur le D6 de l'utilisateur. La release et le tag 0.8.9 restent des archives inchangées.
