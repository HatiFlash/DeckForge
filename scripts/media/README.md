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

## Logo transparent

`python scripts/media/generate-branding.py --preview CHEMIN` régénère les PNG et ICO avec leur canal alpha. Utiliser le PNG utilisateur transparent comme source ; ne jamais ajouter de fond opaque aux icônes.

## Vidéo publiée et ancien pipeline

Le fichier publié `docs/media/novydeck-demo.mp4` est maintenant un film de présentation de 40 s en 1920 × 1080 à 60 i/s, avec son, réalisé hors du dépôt avec Remotion et three.js : pavé D6 en 3D, captures réelles de l'interface et musique synthétisée. Le pipeline [remotion/](remotion/README.md) du dépôt reste disponible pour la présentation précédente ; il ne produit pas ce film publié.

La procédure de rendu décrite dans `remotion/README.md` concerne cette présentation précédente, et non la vidéo actuellement publiée. Les anciens pipelines `promo/` (HTML/Playwright) et `build-demo-video.sh` (fondus FFmpeg) restent conservés comme outils historiques.
