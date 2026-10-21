# Médias de présentation (captures et vidéo)

Tout est reproductible depuis le dépôt. Aucune image n'est générée par IA ni retouchée. Les deux médias du profil de démonstration (un paysage et un égaliseur animé) sont dessinés par le code de `src/demo_profile.rs`.

## Captures du README (Windows)

```powershell
$env:DECKFORGE_UI_OUTPUT = "C:\chemin\captures"
cargo test --locked --bin deckforge -- readme_captures_from_the_demo_profile --test-threads=1
```

Le test produit quatre fichiers à 2560 × 1600 pixels : `deckforge-main.png`, `deckforge-block-editor.png`, `deckforge-assistant.png` (réponse réelle de l'assistant local, aperçu sans exécution) et `deckforge-media-crop.png`. Il faut ensuite les recompresser sans perte, puis les copier dans `docs/images/`.

## Vidéo de présentation

1. Images clés à 1920 × 1080 pixels, prises dans des états réels de l'interface (profil de démonstration, page 2, touche photo, atelier, bloc ajouté puis réglé, assistant, recadrage) :

   ```powershell
   $env:DECKFORGE_VIDEO_OUTPUT = "C:\chemin\frames"
   cargo test --locked --bin deckforge -- demo_video_keyframes --ignored --test-threads=1
   ```

2. Montage avec FFmpeg (Linux, macOS ou WSL). Outils nécessaires : `ffmpeg`/`ffprobe` 6 ou plus récent (avec libx264 et drawtext), `fc-match` et la police Inter (SIL Open Font License).

   ```bash
   scripts/media/build-demo-video.sh /chemin/frames docs/media/deckforge-demo.mp4
   ```

**Sortie :** MP4 H.264 en 1920 × 1080 à 30 i/s, d'environ 30 secondes, avec `+faststart`, sans son ni musique. Une vignette `deckforge-demo-poster.png` est créée à côté.

**Animations :** fondus enchaînés de 0,5 s, rapprochement de 2 % par scène et légendes en fondu. Il n'y a ni zoom brusque ni effet lumineux.

Version des outils lors du dernier rendu : FFmpeg 7.1.5 (Debian 13), Inter 4.x.
