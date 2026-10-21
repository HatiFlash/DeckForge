[English](README.md) · Français

<div align="center">
  <img src="res/branding/novydeck-logo.png" width="120" alt="Logo NovyDeck">
  <h1>NovyDeck</h1>
  <p>Logiciel libre pour Windows qui pilote les stream decks : touches, macros, médias et plugins, sans le logiciel officiel. Testé sur le FIFINE AmpliGame D6 et l'Elgato Stream Deck.</p>
  <p>
    <a href="https://github.com/HatiFlash/NovyDeck/releases/tag/v0.8.14"><img src="https://img.shields.io/badge/Release-v0.8.14-3478F6.svg" alt="Version 0.8.14"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/Licence-MIT-22C58B.svg" alt="Licence MIT"></a>
    <img src="https://img.shields.io/badge/Windows-10%20%2F%2011-555.svg" alt="Windows 10/11">
  </p>
</div>

![Profil de démonstration NovyDeck sur les 15 touches du D6](docs/images/novydeck-main.png)

<sub>Capture réelle de l'interface, rendue hors écran à 2560 × 1600 avec le profil « NovyDeck Demo / README » fourni avec l'application, sans périphérique connecté.</sub>

## Vidéo de présentation

[![Voir le film de présentation NovyDeck](docs/media/novydeck-demo-poster.png)](docs/media/novydeck-demo.mp4)

Un film de présentation de 40 s en 1920 × 1080 à 60 i/s, avec son, réalisé avec Remotion et three.js. Il montre le pavé D6 en 3D, de vraies captures de l'interface et une musique synthétisée. Le film publié a été produit hors du dépôt.

## Points forts

- **15 touches LCD personnalisables :** icônes, couleurs, images, GIF et vidéos courtes, avec un aperçu fidèle à l'écran du D6.
- **Profils, pages et dossiers :** changement automatique de profil selon l'application au premier plan.
- **Atelier de blocs :** macros sans script (raccourcis, texte, applications, médias, fenêtres, conditions, répétitions, variables) et simulation avant exécution réelle.
- **Assistant local :** une phrase en français ou en anglais devient des blocs présentés en aperçu. Rien n'est inséré sans validation ; l'IA locale facultative est désactivée par défaut.
- **Plugins Stream Deck / OpenAction :** import des archives `.streamDeckPlugin` et `.sdPlugin`, réglages HTML isolés ([compatibilité](docs/PLUGINS-COMPATIBILITE.md)).
- **Serveur MCP :** utilisation avec Claude, Codex ou Cursor et validation humaine de chaque modification proposée ([guide MCP](docs/MCP.md)).
- **Bibliothèque communautaire de plugins :** parcourez et installez les plugins partagés par la communauté depuis [novydeck-plugins](https://github.com/HatiFlash/novydeck-plugins). NovyDeck ne se connecte qu'avec votre accord, demande confirmation et vérifie l'empreinte SHA-256 de chaque téléchargement ; les plugins communautaires sont signalés comme non vérifiés.
- **Rapport matériel :** vous testez une autre marque de deck ? **Signaler un problème avec mon deck** prépare un rapport anonymisé (avec aperçu) et ouvre une issue GitHub préremplie ; il suffit d'indiquer le modèle du deck et ce qui ne va pas.

## Compatibilité

| Élément | État |
|---|---|
| Windows 10 et 11, 64 bits | Pris en charge ; installateur par utilisateur ou ZIP portable |
| FIFINE AmpliGame D6 (15 touches) | Pris en charge et testé ; périphérique principal |
| Elgato Stream Deck | Testé |
| Autres decks sur base Mirabox (Mirabox, AJAZZ / Stream Dock…) | Devrait fonctionner ; pas encore testé |
| Plugins Stream Deck | Plugins non chiffrés pris en charge ; plugins Elgato chiffrés non pris en charge |
| IA locale (llama.cpp ou Ollama) | Facultative ; désactivée par défaut |
| macOS, Linux | Non pris en charge |

NovyDeck est conçu pour être compatible avec tous les decks. Pour l'instant, il a été testé sur le **FIFINE AmpliGame D6** et l'**Elgato Stream Deck**. Le D6 reposant sur une base matérielle Mirabox, les autres marques qui utilisent cette même base devraient aussi être compatibles, mais elles n'ont pas encore été testées : vos retours sont les bienvenus : utilisez **Signaler un problème avec mon deck** dans l'application, ou ouvrez une [issue](https://github.com/HatiFlash/NovyDeck/issues/new?template=hardware-report.yml).

Fermez FIFINE Control Deck avant d'utiliser le D6 : ce logiciel conserve l'accès USB exclusif au périphérique.

## Démarrage rapide / installation

1. Téléchargez **`NovyDeck-0.8.14-Setup.exe`** depuis la [dernière version](https://github.com/HatiFlash/NovyDeck/releases/latest) et lancez-le sans droits administrateur, ou extrayez `NovyDeck-0.8.14-Windows.zip`. Une nouvelle installation utilise `%LOCALAPPDATA%\Programs\NovyDeck`.
2. Pour une mise à jour, l'installateur met à jour l'installation existante en conservant profils, médias et plugins. En version portable, fermez l'application et remplacez les exécutables sans supprimer `config.json`, les préférences, `assets` ni `plugins`.
3. Lancez NovyDeck depuis le menu Démarrer (ou le dossier portable) et branchez le D6. Pour découvrir l'application sans toucher à vos profils, choisissez **Profils → ••• → Profil de démonstration (15 touches)**. Cette commande réinitialise aussi le profil de démonstration ; vous pouvez le supprimer comme tout autre profil.
4. Glissez une action de la bibliothèque sur une touche, ou ouvrez l'atelier de blocs.

## Galerie

| Atelier de blocs | Assistant local |
|---|---|
| ![Macro de cinq blocs et propriétés du raccourci](docs/images/novydeck-block-editor.png) | ![Aperçu d'une macro conditionnelle proposée par les règles locales](docs/images/novydeck-assistant.png) |
| Modifiez le bloc sélectionné dans le panneau de droite. | Prévisualisez une condition avant son ajout. |

| Recadrage des médias | Coins arrondis des touches |
|---|---|
| ![Aperçu du recadrage pour une touche du D6](docs/images/novydeck-media-crop.png) | ![Avant](docs/images/key-corners-before.png) ![Après](docs/images/key-corners-after.png) |
| Aperçu sur la touche sans altérer le fichier d'origine. | Les fonds épousent désormais l'arrondi de chaque touche. |

| Réglages de plugin : Elgato | Réglages de plugin : VolumeController |
|---|---|
| ![Inspecteur de propriétés Elgato](docs/pi-product.png) | ![Inspecteur de propriétés VolumeController](docs/pi-volume.png) |
| ![Réglages SignalRGB](docs/pi-signalrgb.png) | ![Réglages du compteur Elgato](docs/pi-counter.png) |

Les captures des plugins utilisent des données du banc UI, et non des plugins installés par l'utilisateur.

## Fonctionnalités en détail

### Périphériques

Le **FIFINE AmpliGame D6** est le contrôleur principal : 15 touches LCD avec retour visuel, réglage de la luminosité et mise en veille automatique. L'**Elgato Stream Deck** a également été testé, avec un pilote HID direct, la synchronisation des touches et le retour d'état. Les autres contrôleurs sur base Mirabox (Mirabox, AJAZZ / Stream Dock…) partagent la plateforme matérielle du D6 et devraient fonctionner, mais n'ont pas encore été testés.

### Atelier de blocs

Composez des automatisations en glissant des blocs dans l'interface, sans AutoHotkey ni scripts.

![Atelier de blocs NovyDeck](docs/workflow.png)

- **Audio :** réglez le volume de l'application au premier plan ou d'un exécutable précis sans modifier le volume général de Windows ; basculez entre casque et enceintes avec mise à jour immédiate de l'icône et du titre de la touche.
- **Streaming et médias :** pilotez scènes, enregistrement, diffusion, replay buffer, sources et micro d'OBS Studio ; utilisez Discord, la soundboard intégrée et les commandes multimédias.
- **Logique :** les blocs si/alors/sinon s'appuient sur 14 conditions en temps réel, notamment fenêtre active, audio en cours, micro muet, casque branché et variables globales ; un bloc « Attendre que… » est aussi disponible.
- **Système et fenêtres :** déplacez ou ancrez les fenêtres, activez-les au premier plan, simulez des touches maintenues avec anti-rebond et déplacez la souris relativement.
- **Variables et déclencheurs :** réagissez à l'ouverture ou à la fermeture d'une application, à une heure donnée ou à un événement audio ; insérez `{heure}`, `{date}` et `{presse_papiers}` dans vos messages et requêtes.

### Plugins Stream Deck et OpenAction

Importez les archives `.streamDeckPlugin` et `.sdPlugin` depuis le magasin intégré, un fichier ou un dossier. Dans **Bibliothèque → Plugins**, cliquez sur **Importer ce plugin** sur une carte, ou utilisez **Importer tout**.

![Carte de plugin issue d'un banc UI](docs/plugins-store.png)

Les exécutables natifs C++/Rust/C#, les scripts Node.js (`@elgato/streamdeck` v2) et les plugins avec moteur HTML (`Runtime::Html`) sont pris en charge. Les inspecteurs de propriétés HTML s'exécutent dans un sous-processus WebView2 isolé (`--isolated-inspector`) derrière un proxy loopback restreint. Les composants `sdpi-components` v3/v4 et BarRaider EasyPI sont embarqués hors ligne, avec intégration du thème sombre. Les réglages se synchronisent dans les deux sens (`setSettings`, `setGlobalSettings`) entre l'interface et le matériel. Les plugins Elgato chiffrés ne sont pas pris en charge.

### Médias et recadrage

Recadrez images et vidéos sans modifier les originaux avant de les envoyer sur une touche : modes Remplir ou Ajuster/letterbox, zoom 0,5×–3,0×, recentrage et aperçu instantané sur la touche. Media Foundation sous Windows décode les vidéos courtes sans FFmpeg ; les GIF en boucle sont pris en charge. La galerie accepte les images PNG, JPEG, WebP et SVG transparents ainsi que les animations.

### Fiabilité et import Elgato

NovyDeck conserve les cinq dernières sauvegardes rotatives saines (`backups/config-*.bak`) et propose une restauration guidée sans écraser les données valides avec un fichier corrompu. Importez en une étape une archive Elgato `.StreamDeckProfilesBackup` complète, avec détection et conversion automatiques de ses profils. La simulation de macros ne provoque aucun effet de bord système non sollicité.

### Intégration MCP

Le serveur Model Context Protocol intégré se connecte à Claude Desktop, Claude Code, Codex CLI ou Cursor depuis la barre d'outils IA. Ces assistants peuvent explorer les profils et proposer des dispositions de touches et des macros ; chaque modification proposée apparaît en aperçu et exige une validation explicite avant application. La connexion en un clic ne nécessite aucune édition manuelle de configuration JSON.

### Interface et rendu

L'interface utilise le rendu matériel wgpu (DirectX 12 / Vulkan, avec repli OpenGL). La grille des touches s'adapte à la taille de la fenêtre et à la résolution de l'écran. Les touches LCD peuvent afficher des images, des GIF animés et de courtes vidéos en boucle. Les profils sont organisables en dossiers et peuvent changer automatiquement au lancement d'une application.

## Confidentialité

NovyDeck fonctionne hors ligne. L'IA locale reste sur votre PC ; elle est facultative et désactivée par défaut. L'IA connectée et les plugins réseau ne s'activent qu'avec votre accord explicite. Les actions proposées par un assistant s'affichent en aperçu avant insertion ou application.

## Documentation

[Identité visuelle](docs/DESIGN.md) · [MCP et IA](docs/MCP.md) · [Architecture des plugins](docs/PLUGINS-ARCHITECTURE.md) · [Compatibilité des plugins](docs/PLUGINS-COMPATIBILITE.md) · [Import Elgato](docs/ELGATO.md) · [Tests périphériques](docs/TEST-PERIPHERIQUES.md) · [Guide des médias](scripts/media/README.md)

## Dernière version

- **0.8.14 :** mise à jour depuis l'application : **À propos → Vérifier les mises à jour** interroge la dernière release GitHub ; **Télécharger et installer** vérifie l'empreinte SHA-256 de l'installateur, ferme NovyDeck, installe par-dessus (profils, médias et plugins conservés) puis le relance. Les copies portables sont renvoyées vers la page de téléchargement.
- **0.8.13 :** l'action **Basculer l'audio** peut être déposée sur une touche avant de choisir ses deux périphériques, réglés ensuite dans l'inspecteur ; elle était auparavant refusée avec « Choisis deux périphériques audio différents ».
- **0.8.12 :** bibliothèque communautaire de plugins (Bibliothèque → Plugins → Communauté), rapport matériel anonymisé avec issue GitHub préremplie, et le bouton **Recadrer** sous l'aperçu de la touche n'est plus coupé.
- **0.8.11 :** inscription NOVYDECK sur le châssis, avec test UI et actualisation des captures et de la vidéo.
- **0.8.10 :** préservation de la transparence du logo utilisateur dans les PNG et toutes les tailles d'icônes Windows ; génération reproductible des ressources et test de non-régression du canal alpha.

Consultez les [versions publiées sur GitHub](https://github.com/HatiFlash/NovyDeck/releases) pour les notes complètes.

## Compiler depuis les sources

NovyDeck est développé en Rust. La compilation et les tests utilisent les dépendances verrouillées ; la création de l'installateur Windows nécessite Inno Setup 6 ou 7.

```bash
git clone https://github.com/HatiFlash/NovyDeck.git
cd NovyDeck
cargo build --release --locked
cargo test --locked
powershell -ExecutionPolicy Bypass -File tools\Build-Installer.ps1 -Version 0.8.14 -Build
```

## Licence et crédits

NovyDeck est distribué sous [licence MIT](LICENSE). Le dépôt source ne contient aucun binaire tiers propriétaire, aucune archive protégée ni aucun pack de ressources fermé. Les icônes d'interface proviennent d'[IconPark](https://github.com/bytedance/IconPark) (ByteDance), sous licence Apache 2.0 ; seuls les pictogrammes utilisés sont embarqués et recolorés pour NovyDeck ([notice](res/iconpark/NOTICE.md), [licence](res/iconpark/LICENSE)). Consultez [THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt) pour les mentions relatives aux dépendances libres et bibliothèques embarquées.
