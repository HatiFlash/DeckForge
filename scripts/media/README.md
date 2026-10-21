# Médias NovyDeck : captures et vidéo

Les captures viennent de l'application réelle, rendue en banc sans périphérique connecté. Aucun écran ni fonction inexistante n'est maquillé. Les deux médias du profil démo (paysage et égaliseur animé) sont dessinés localement par `src/demo_profile.rs`. Le logo NovyDeck est celui fourni par Hatim.

## Captures du README (Windows)

```powershell
$env:NOVYDECK_UI_OUTPUT = "C:\chemin\captures"
cargo test --locked --bin novydeck readme_captures_from_the_demo_profile -- --test-threads=1
```

Quatre PNG 2560 × 1600 sont générés : `novydeck-main.png`, `novydeck-block-editor.png`, `novydeck-assistant.png` (vraie réponse locale, sans exécution) et `novydeck-media-crop.png`. Les copier dans `docs/images/` et dans `scripts/media/remotion/public/shots/` en retirant le préfixe `novydeck-` pour ce dernier dossier.

## Écrans LCD pour la 3D

```powershell
$env:NOVYDECK_VIDEO_OUTPUT = "C:\chemin\frames"
cargo test --locked --bin novydeck demo_video_keyframes -- --ignored --test-threads=1
```

Le dossier `keys/` contient les 15 écrans LCD rendus par `key_render` (252 px) et les 12 photogrammes de la touche animée. Les recopier dans `remotion/public/keys/`. Les anciennes variables d'environnement `DECKFORGE_*_OUTPUT` restent acceptées.

## Vidéo 0.8.9 : Remotion

**Pipeline actuel : [remotion/README.md](remotion/README.md).** 43 s, 1920 × 1080, 30 i/s, H.264 et AAC. Sept chapitres, scène de pavé en vraie 3D, mouvements déterministes, captures réelles, nouveau logo et son procédural synchronisé aux transitions. Le fichier publié est `docs/media/novydeck-demo.mp4`.

Le rendu de publication a été exécuté sur le portable Windows de Hatim avec ANGLE et l'Iris Xe. TypeScript et les ressources ont été contrôlés avant rendu ; des images du MP4 final ont été relues.

Les anciens pipelines `promo/` (HTML/Playwright) et `build-demo-video.sh` (fondus FFmpeg) sont conservés comme outils historiques ; ils ne produisent pas le film Remotion livré en 0.8.9.
