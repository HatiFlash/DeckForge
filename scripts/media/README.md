# Médias de présentation (captures et vidéo)

Tout est reproductible depuis le dépôt. Aucune image n'est générée par IA ni retouchée. Les deux médias du profil de démonstration (un paysage et un égaliseur animé) sont dessinés par le code de `src/demo_profile.rs`.

## Captures du README (Windows)

```powershell
$env:DECKFORGE_UI_OUTPUT = "C:\chemin\captures"
cargo test --locked --bin deckforge -- readme_captures_from_the_demo_profile --test-threads=1
```

Le test produit quatre fichiers à 2560 × 1600 pixels : `deckforge-main.png`, `deckforge-block-editor.png`, `deckforge-assistant.png` (réponse réelle de l'assistant local, aperçu sans exécution) et `deckforge-media-crop.png`. Il faut ensuite les recompresser sans perte, puis les copier dans `docs/images/`.

## Vidéo de présentation

La vidéo `docs/media/deckforge-demo.mp4` (42 s, 1920 × 1080, 30 i/s, H.264 + AAC) est une scène three.js rendue image par image : pavé D6 en 3D avec les écrans réels des 15 touches, caméra qui traverse les vraies captures de l'application, typographie animée et bande-son procédurale.

1. Écrans des touches et images clés, issus de l'application :

   ```powershell
   $env:DECKFORGE_VIDEO_OUTPUT = "C:\chemin\frames"
   cargo test --locked --bin deckforge -- demo_video_keyframes --ignored --test-threads=1
   ```

   Le dossier `keys/` contient les 15 écrans LCD rendus par `key_render` (252 px) et les 12 images de la touche animée.

2. Rendu : voir [promo/README.md](promo/README.md) (Playwright + Chromium, FFmpeg). Avec un GPU (ANGLE D3D11), le rendu complet prend environ 10 minutes sur un portable Iris Xe.

L'ancien montage par fondus (`build-demo-video.sh`) reste disponible pour une vidéo simple sans 3D.
