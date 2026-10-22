[English](README.md) · Français

<div align="center">
  <img src="res/branding/novydeck-logo.png" width="120" alt="Logo NovyDeck">
  <h1>NovyDeck</h1>
  <p>Logiciel libre pour Windows qui pilote les stream decks : touches, macros, médias et plugins, sans le logiciel officiel. Testé sur le FIFINE AmpliGame D6 et l'Elgato Stream Deck.</p>
  <p>
    <a href="https://github.com/HatiFlash/NovyDeck/releases/tag/v0.9.2"><img src="https://img.shields.io/badge/Release-v0.9.2-3478F6.svg" alt="Version 0.9.2"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/Licence-MIT-22C58B.svg" alt="Licence MIT"></a>
    <img src="https://img.shields.io/badge/Windows-10%20%2F%2011-555.svg" alt="Windows 10/11">
  </p>
</div>

![Profil de démonstration NovyDeck sur les 15 touches du D6](docs/images/novydeck-main.png)

<sub>Capture réelle de l'interface, rendue hors écran à 2560 × 1600 avec le profil « NovyDeck Demo / README » fourni avec l'application, sans périphérique connecté.</sub>

## Vidéo de présentation

[![Voir le film de présentation NovyDeck](docs/media/novydeck-demo-poster.png)](docs/media/novydeck-demo.mp4)

Un film de présentation de 40 s en 1920 × 1080 à 60 i/s, avec son, réalisé avec Remotion et three.js. Il montre le pavé D6 en 3D, de vraies captures de l'interface et une musique synthétisée. Le film publié a été produit hors du dépôt.

## Nouveautés 0.9.2

### Deux images par touche : marche / arrêt

Sélectionne une touche, puis **Apparence → Adapter la touche à deux états → Interrupteur : change à chaque appui**. Choisis séparément les images **Activé** et **Désactivé** avec **Choisir une image…** ou la galerie. Chaque appui court physique alterne les deux images tout en exécutant l’action habituelle ; un appui long configuré ne change pas l’image. L’état revient à **Désactivé** au démarrage ou après un changement de page/profil. Les touches existantes à image unique ne changent pas.

![Panneau Apparence avec images Activé et Désactivé](docs/images/key-toggle-fr.png)

### Autres corrections

La fenêtre **Windows et raccourcis** reste centrée et fixe. La barre d’état, le titre de l’Atelier et le journal de simulation respectent désormais la langue anglaise choisie. Les plugins officiels 1.0.1 retirent les cadres noirs autour du texte des touches ; les captures CPU et RAM utilisent des mesures d’un agent Windows de compilation (les données GPU indisponibles restent indiquées comme telles).

## Nouveautés 0.9.0

### Magasin de plugins

Ouvrez **Bibliothèque → Plugins → Magasin de plugins**. Le magasin est une page de l'application, comme l'Atelier de blocs : recherchez, filtrez par catégorie, ouvrez un plugin pour lire sa description, voir ses captures et ses prérequis, puis cliquez sur **Installer**. L'archive n'est téléchargée qu'après votre confirmation et son empreinte SHA-256 est vérifiée avant l'import.

| Magasin | Fiche d'un plugin |
|---|---|
| ![Magasin de plugins NovyDeck avec plugins officiels et open source](docs/images/plugin-store-fr.png) | ![Fiche d'un plugin avec captures et bouton Installer](docs/images/plugin-store-detail-fr.png) |

Disponibles dès maintenant : sept **plugins NovyDeck officiels** — Horloge, Compteur, Moniteur système, Lecture en cours, Économiseur OLED, Home Assistant et **Commande Google Assistant** (pilotez vos appareils Google Home avec une commande écrite comme « allume la lumière du salon ») — ainsi que des plugins open source testés dans NovyDeck (Elgato CPU, BarRaider Stopwatch).

### Actions de plugins dans l'Atelier de blocs

La palette **Plugins** liste les actions de chaque plugin installé. Enchaînez des actions de **plugins différents** dans une même macro, avec des attentes, des conditions et tous les autres blocs. L'assistant local comprend les noms des plugins et n'invente jamais une action non installée.

![Macro qui enchaîne NovyDeck Clock et Commande Google Assistant](docs/images/workshop-plugins-fr.png)

### Démarrer réduit

Quand NovyDeck se lance avec Windows, il peut maintenant rester réduit près de l'horloge sans afficher sa fenêtre (case sous **Lancer NovyDeck au démarrage de Windows**, dans **Windows et raccourcis…**).

![Option Démarrer réduit](docs/images/start-minimized-fr.png)

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

1. Téléchargez **`NovyDeck-0.9.2-Setup.exe`** depuis la [dernière version](https://github.com/HatiFlash/NovyDeck/releases/latest) et lancez-le sans droits administrateur, ou extrayez `NovyDeck-0.9.2-Windows.zip`. Une nouvelle installation utilise `%LOCALAPPDATA%\Programs\NovyDeck`.
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
- **Actions de plugins :** la palette **Plugins** liste les actions de chaque plugin installé ; enchaînez des actions de plugins différents dans une même macro, avec n'importe quel autre bloc. Chaque bloc garde ses propres réglages, et l'assistant local comprend les noms des plugins (« ajoute le chronomètre de NovyDeck Clock ») sans jamais inventer une action non installée.
- **Variables et déclencheurs :** réagissez à l'ouverture ou à la fermeture d'une application, à une heure donnée ou à un événement audio ; insérez `{heure}`, `{date}` et `{presse_papiers}` dans vos messages et requêtes.

### Plugins Stream Deck et OpenAction

Importez les archives `.streamDeckPlugin` et `.sdPlugin` depuis le magasin intégré, un fichier ou un dossier. Dans **Bibliothèque → Plugins**, cliquez sur **Importer ce plugin** sur une carte, ou utilisez **Importer tout**.

![Carte de plugin issue d'un banc UI](docs/plugins-store.png)

Les exécutables natifs C++/Rust/C#, les scripts Node.js (`@elgato/streamdeck` v2) et les plugins avec moteur HTML (`Runtime::Html`) sont pris en charge. Les inspecteurs de propriétés HTML s'exécutent dans un sous-processus WebView2 isolé (`--isolated-inspector`) derrière un proxy loopback restreint. Les composants `sdpi-components` v3/v4 et BarRaider EasyPI sont embarqués hors ligne, avec intégration du thème sombre. Les réglages se synchronisent dans les deux sens (`setSettings`, `setGlobalSettings`) entre l'interface et le matériel. Les plugins Elgato chiffrés ne sont pas pris en charge.

### Magasin de plugins et bibliothèque communautaire

Ouvrez **Bibliothèque → Plugins**. Le bouton **Magasin de plugins** apparaît en premier, avant les plugins installés. Il ouvre une fenêtre redimensionnable : après votre accord pour la connexion à GitHub, recherchez et filtrez les plugins, consultez leurs descriptions et captures, puis cliquez sur **Installer** dans la fiche détaillée. Les imports locaux restent disponibles hors ligne sous ce bouton.

1. **Installés dans NovyDeck** : les plugins déjà importés depuis un fichier, un dossier ou le catalogue intégré. Glissez leurs actions sur une touche comme n'importe quelle autre action.
2. **Communauté** : un annuaire public de plugins maintenu dans [HatiFlash/novydeck-plugins](https://github.com/HatiFlash/novydeck-plugins).

**Plugins NovyDeck officiels** (natifs, sans Node.js, compatibles aussi avec le logiciel Elgato ; sources dans [first-party/](https://github.com/HatiFlash/novydeck-plugins/tree/main/first-party)) : **Horloge** (horloge, minuteur, chronomètre), **Compteur** (compteur, note rapide, presse-papiers), **Moniteur système** (CPU, RAM, GPU), **Lecture en cours** (titre et pochette du morceau), **Économiseur OLED** (écran noir sur le moniteur choisi), **Home Assistant** (basculer des entités, appeler des services, afficher des capteurs) et **Commande Google Assistant** (envoie une commande écrite comme « allume la lumière du salon » à vos appareils Google Home, avec votre propre projet Google Cloud). Des plugins open source sélectionnés (Elgato CPU, BarRaider Stopwatch) sont listés après avoir été testés dans NovyDeck.

Fonctionnement de la bibliothèque communautaire :

- NovyDeck reste hors ligne tant que vous n'avez pas cliqué sur **Se connecter au magasin** ; il télécharge alors uniquement le catalogue (`registry.json`) depuis GitHub.
- Chaque carte indique le nom, la version, l'auteur, la licence, les decks testés et la version minimale de NovyDeck. Les entrées portent la mention **Communauté — non vérifié**.
- **Installer…** ouvre une confirmation avec l'adresse de téléchargement et l'empreinte SHA-256 attendue. NovyDeck télécharge l'archive, vérifie son empreinte et l'importe seulement ensuite. Une empreinte identique prouve que le fichier est bien celui référencé, pas que le code tiers est sûr : n'installez que des plugins de confiance.

```mermaid
flowchart LR
  A[Auteur du plugin] -->|pull request| B[Registre novydeck-plugins]
  B -->|CI : schéma, téléchargement, SHA-256, contrôle ZIP| C[Relecture et fusion par le mainteneur]
  C --> D[registry.json]
  D -->|après votre accord| E[NovyDeck : Bibliothèque → Plugins → Communauté]
  E -->|confirmation + SHA-256 vérifié| F[Plugin importé]
```

**Partager votre plugin :** publiez un `.streamDeckPlugin` (ou un `.zip` contenant un dossier `.sdPlugin`) à une adresse HTTPS stable, calculez son SHA-256 (`certutil -hashfile fichier SHA256` sous Windows), ajoutez une entrée à `registry.json` selon le [schéma](https://github.com/HatiFlash/novydeck-plugins/blob/main/schema/registry.schema.json) et ouvrez une pull request. La CI vérifie l'entrée automatiquement et le mainteneur décide de la publication. Guide complet : [CONTRIBUTING.fr.md](https://github.com/HatiFlash/novydeck-plugins/blob/main/CONTRIBUTING.fr.md). Le bouton **Proposer un plugin** de l'application ouvre la même page.

### Médias et recadrage

Recadrez images et vidéos sans modifier les originaux avant de les envoyer sur une touche : modes Remplir ou Ajuster/letterbox, zoom 0,5×–3,0×, recentrage et aperçu instantané sur la touche. Media Foundation sous Windows décode les vidéos courtes sans FFmpeg ; les GIF en boucle sont pris en charge. La galerie accepte les images PNG, JPEG, WebP et SVG transparents ainsi que les animations.

### Fiabilité et import Elgato

NovyDeck conserve les cinq dernières sauvegardes rotatives saines (`backups/config-*.bak`) et propose une restauration guidée sans écraser les données valides avec un fichier corrompu. Importez en une étape une archive Elgato `.StreamDeckProfilesBackup` complète, avec détection et conversion automatiques de ses profils. La simulation de macros ne provoque aucun effet de bord système non sollicité.

### Intégration MCP

Le serveur Model Context Protocol intégré se connecte à Claude Desktop, Claude Code, Codex CLI ou Cursor depuis la barre d'outils IA. Ces assistants peuvent explorer les profils et proposer des dispositions de touches et des macros ; chaque modification proposée apparaît en aperçu et exige une validation explicite avant application. La connexion en un clic ne nécessite aucune édition manuelle de configuration JSON.

### Interface et rendu

L'interface utilise le rendu matériel wgpu (DirectX 12 / Vulkan, avec repli OpenGL). La grille des touches s'adapte à la taille de la fenêtre et à la résolution de l'écran. Les touches LCD peuvent afficher des images, des GIF animés et de courtes vidéos en boucle. Les profils sont organisables en dossiers et peuvent changer automatiquement au lancement d'une application.

## Langues

L'interface est disponible en **français** et en **anglais**. Au premier démarrage, NovyDeck suit la langue de Windows ; changez-la dans **Réglages → Langue**. Chaque langue est un simple fichier JSON dans [`res/i18n/`](res/i18n) : pour ajouter la vôtre, copiez `en.json`, traduisez les valeurs et ouvrez une pull request. Voir le [guide de traduction](docs/TRANSLATING.md).

## Confidentialité

NovyDeck fonctionne hors ligne. L'IA locale reste sur votre PC ; elle est facultative et désactivée par défaut. L'IA connectée et les plugins réseau ne s'activent qu'avec votre accord explicite. Les actions proposées par un assistant s'affichent en aperçu avant insertion ou application.

## Documentation

[Identité visuelle](docs/DESIGN.md) · [MCP et IA](docs/MCP.md) · [Architecture des plugins](docs/PLUGINS-ARCHITECTURE.md) · [Compatibilité des plugins](docs/PLUGINS-COMPATIBILITE.md) · [Import Elgato](docs/ELGATO.md) · [Tests périphériques](docs/TEST-PERIPHERIQUES.md) · [Guide des médias](scripts/media/README.md)

## Dernière version

- **0.9.2 :** deux images Activé/Désactivé par touche alternent à l’appui court physique ; textes anglais et panneau Windows fixe ; plugins officiels 1.0.1 sans cadres noirs, aperçus CPU/RAM mesurés.
- **0.9.1 :** le magasin de plugins devient une page fixe de l'application (plus de fenêtre flottante), avec bouton retour et Échap ; les blocs de plugins de l'Atelier affichent les noms lisibles du plugin et de l'action ; le README présente les nouveautés 0.9.0 avec captures.
- **0.9.0 :** fenêtre **Magasin de plugins** façon Play Store (recherche, catégories, fiche avec captures, installation en un clic avec contrôle SHA-256) ; sept plugins NovyDeck officiels et des plugins open source testés ; **actions de plugins dans l'Atelier de blocs**, combinables entre plugins et comprises par l'assistant local ; option **démarrer réduit** quand NovyDeck se lance avec Windows.
- **0.8.15 :** interface en anglais avec choix de la langue (**Réglages → Langue**) et traductions communautaires sous forme de simples fichiers JSON ; vérification facultative des mises à jour au démarrage (demandée une fois) ; notes de version affichées avant d'installer une mise à jour et fenêtre **Quoi de neuf** après.
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
powershell -ExecutionPolicy Bypass -File tools\Build-Installer.ps1 -Version 0.9.2 -Build
```

## Licence et crédits

NovyDeck est distribué sous [licence MIT](LICENSE). Le dépôt source ne contient aucun binaire tiers propriétaire, aucune archive protégée ni aucun pack de ressources fermé. Les icônes d'interface proviennent d'[IconPark](https://github.com/bytedance/IconPark) (ByteDance), sous licence Apache 2.0 ; seuls les pictogrammes utilisés sont embarqués et recolorés pour NovyDeck ([notice](res/iconpark/NOTICE.md), [licence](res/iconpark/LICENSE)). Consultez [THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt) pour les mentions relatives aux dépendances libres et bibliothèques embarquées.
