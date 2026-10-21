# Identité NovyDeck

Logo fourni par Hatim pour NovyDeck. La 0.8.10 utilise le fichier transparent corrigé fourni par l'utilisateur, conservé à l'identique comme source.

- `novydeck-source.png` : original utilisateur (1536 × 1024), conservé sans modification.
- `novydeck-logo.png` : déclinaison carrée 1024 × 1024, sans déformation du dessin.
- `novydeck-256.png`, `novydeck-64.png`, `novydeck-32.png` : variantes d'interface.
- `novydeck.ico` : icône Windows multirésolution (16 à 256 px).
- `scripts/media/generate-branding.py` régénère ces fichiers à partir de la source avec Pillow. Il conserve l'alpha, cadre le crop `(240, 120, 1360, 952)` sur un canvas transparent centré, puis redimensionne en LANCZOS. L'option `--preview DOSSIER` produit des aperçus de contrôle sur fonds blanc, gris et foncé.

La source doit contenir à la fois des pixels transparents et visibles; le générateur refuse les sources entièrement opaques ou vides. Les médias personnels des touches ne sont pas remplacés.
