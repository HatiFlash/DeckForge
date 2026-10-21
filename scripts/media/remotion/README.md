# NovyDeck v0.8.10 · film Remotion

43 secondes à 1920 × 1080 / 30 images par seconde. Sept chapitres éditables, six transitions, une vraie scène Three.js de 15 touches LCD (textures issues des touches réelles), caméra orbitale animée image par image par Remotion. La bande-son stéréo 48 kHz / 43 s est générée par `make-sound.py` et synchronisée aux chapitres. Aucun déclenchement de macro ni accès matériel pendant le rendu.

## Ressources indispensables AVANT de rendre

Déposer quatre captures de l'application NovyDeck v0.8.10, issues du logiciel, dans `public/shots/`, EXACTEMENT sous ces noms : `main.png`, `block-editor.png`, `assistant.png`, `media-crop.png`. Les captures NovyDeck validées sont incluses ; les régénérer après toute modification visuelle de l’application. Les vues doivent correspondre aux vrais écrans et être suffisamment nettes (idéalement 2560 × 1600). Aucun écran de remplacement n'est inventé.

`public/logo-original.png` reprend le logo fourni ; `public/logo.png` est un premier cadrage carré 1024 × 1024 du même fichier, validé à partir de la ressource utilisateur. `public/keys/` contient les 15 images des écrans LCD existants et les 12 photogrammes d'une touche animée, effectivement joués dans la scène 3D. Si de nouveaux rendus NovyDeck des touches sont livrés, remplacer les 15 `key-00.png`…`key-14.png` et les 12 `key-09-f00.png`…`key-09-f11.png` sans changer les noms. L'examen OCR des touches existantes n'a pas détecté d'ancien nom de produit, mais vérifier visuellement les assets avant publication.

## Sur Windows (PowerShell, dans ce dossier)

Installer Node.js récent puis :

    npm.cmd install
    npm.cmd run assets
    npm.cmd run typecheck
    npm.cmd run studio

Contrôle d'une image choisie (0 à 1289) :

    npm.cmd run still -- 90
    npm.cmd run still -- 420 out/dashboard.png
    npm.cmd run still -- 1160 out/signature.png

Exporter H.264 (ANGLE utilise la GPU Windows lorsqu'elle est disponible) :

    npm.cmd run render -- out/novydeck-v0.8.10.mp4

Passer des options Remotion supplémentaires après le chemin, par exemple `--concurrency=1`. Avec Iris Xe, démarrer à `--concurrency=1` si la VRAM est insuffisante. Si `--gl=angle` échoue, diagnostiquer le pilote et tester `--gl=swangle` explicitement (logiciel, plus lent) ; ne pas prétendre avoir rendu sur ce portable depuis Linux. Le scénario est déterministe : ne pas utiliser de chronomètre, `useFrame()` ou animation CSS. Aucun réseau n'est requis à l'exécution une fois `npm install` terminé.

## Origine et licences

Inter (SIL Open Font License) : `public/fonts/Inter-LICENSE.txt`. Effets sonores instrumentaux déjà fournis, générés avec un moteur distribué sous licence MIT : `licenses/sfx-MIT-LICENSE.txt`. Logo et captures : ressources du projet NovyDeck, fournis et autorisés pour cette publication par Hatim. L'ancienne vidéo et ses captures ne sont pas insérées dans cette composition. Un `package-lock.json` issu de l’installation réelle Windows est fourni. Les dépendances Remotion sont toutes verrouillées à la même version 4.0.537 ; autres versions exactes dans `package.json`.

Validation de publication : ressources présentes, `tsc --noEmit` réussi, export H.264/AAC sur le portable Windows Iris Xe et revue des images du MP4 final. Rendu image par image sous Remotion 4.0.537.

Pour reconstruire le son : `python make-sound.py` (Python stdlib, moteur `sfx/` sous MIT). Les anciens projets Playwright ne sont pas utilisés pour cet export.
