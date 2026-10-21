# Présentation animée de DeckForge

Vidéo de 42 s (1920 × 1080, 30 i/s, son stéréo) construite à partir d'éléments **réels** de l'application :

- les 15 écrans LCD du profil de démonstration, rendus par le moteur `key_render` de DeckForge
  (test `demo_video_keyframes`, dossier `keys/`), y compris les 12 images de l'animation ;
- les captures `docs/images/deckforge-main.png`, `deckforge-block-editor.png`, `deckforge-assistant.png`
  (réponse réelle de l'assistant local, en aperçu).

Le pavé FIFINE D6 est modélisé en 3D avec three.js (châssis, 15 touches, éclairage PBR, bloom),
la caméra traverse les écrans de l'application. Aucune image générée par IA, aucune retouche des captures.

## Fichiers

- `promo.html` : scène three.js + calques typographiques ; `render(t)` est une fonction pure du temps.
- `timeline.json` : tous les temps forts (image et son lisent la même source).
- `cues.py` + `sfx/` : bande-son procédurale (moteur `sfx` de [claude-motion](https://github.com/whaleyxbt/claude-motion), MIT),
  masterisée à −14 LUFS.
- `render.py` : rendu image par image (Playwright + Chromium) puis encodage H.264 avec FFmpeg.

Règles de mise en mouvement suivies : skill *motion-design* de claude-motion (courbes d'accélération,
entrées décalées, un seul moment fort, grain et vignette, temps de lecture).

## Reproduire

Prérequis : Python 3.11+, `pip install playwright imageio-ffmpeg`, `python -m playwright install chromium`,
three.js 0.186.1 décompressé dans `three/` (`npm pack three@0.186.1`), et les ressources dans `assets/` :

```bash
cargo test --bin deckforge -- demo_video_keyframes --ignored   # DECKFORGE_VIDEO_OUTPUT=…/video
cp -r …/video/keys assets/keys
cp ../../../docs/images/deckforge-{main,block-editor,assistant}.png assets/shots/
cp ../../../res/branding/deckforge-logo.png assets/
python -m sfx cues.py                                            # out/sfx.wav
ANGLE=d3d11 FFMPEG=$(python -c "import imageio_ffmpeg as f;print(f.get_ffmpeg_exe())") \
  python render.py video out/video.mp4                           # ANGLE=swiftshader sans GPU (très lent)
ffmpeg -i out/video.mp4 -i out/sfx.wav -map 0:v -map 1:a -c:v copy -c:a aac -b:a 192k -shortest \
  -movflags +faststart ../../../docs/media/deckforge-demo.mp4
```

`python render.py stills 1,9,15,23,31,38 planche.png` produit une planche de contrôle.
Polices : Inter et DejaVu Sans Mono (SIL OFL / licence DejaVu, fichiers de licence dans `fonts/`).
