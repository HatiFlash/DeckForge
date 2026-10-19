# DeckForge 0.7.0 — version de test Windows

<img src="res/branding/deckforge-logo.png" width="96" align="right" alt="Logo DeckForge">

Application Rust indépendante pour le FIFINE AmpliGame D6 (USB 3142:0060) : macros, pages, dossiers, Atelier de blocs, actions natives (audio, OBS, Discord, système) et connecteur MCP optionnel. Le logiciel FIFINE doit être fermé pour laisser DeckForge accéder au D6.

![Interface](docs/interface.png)

Pour connecter Claude Desktop, Claude Code, Codex CLI ou Cursor sans éditer de fichier : bouton **IA** dans la barre → **Connecter une IA**. Voir [le guide MCP](docs/MCP.md).

## État de la 0.7.0 — 18 octobre 2026

DeckForge peut importer des archives de plugins Stream Deck/OpenAction et héberger des actions natives ou Node.js. L'hôte suit les contrats communs de lancement, WebSocket, contextes et réglages ; les mécanismes d'[OpenDeck/OpenAction](docs/PLUGINS-OPENDECK-COMPARAISON.md) ont servi de référence, **sans copier de code OpenDeck GPL**.

**Essai confirmé d'un plugin tiers :** l'archive OpenAction Counter 1.0.1 a été importée par le code d'import dans un dossier temporaire, puis son action a fonctionné dans une App isolée avec appareil virtuel. Dans une copie indépendante de DeckForge sur le FIFINE D6, un appui sur la touche 1 a provoqué `keyDown`/`keyUp` ; le véritable plugin a renvoyé `setSettings` et `setTitle`, le compteur sauvegardé est passé de 0 à 1 et l'utilisateur a vu **1 sur la touche physique**. Ce test matériel utilisait un package extrait de façon contrôlée : **le bouton d'import de l'interface n'a pas été testé lors de cet essai**. Aucun binaire tiers n'est inclus dans le dépôt.

**Banc Windows virtuel (19 octobre 2026) :** cinq plugins tiers s'enregistrent et réagissent à un appui simulé : trois exemples officiels Elgato en Node.js (SDK `@elgato/streamdeck` v2 : counter, hello-world, lights-out) et deux plugins natifs OpenAction (Counter, System). Le compteur Elgato et OpenAction Counter retrouvent leur valeur après un redémarrage du plugin et une relecture de la configuration. Un plugin importé depuis le magasin est maintenant utilisable **sans relancer DeckForge**. Détails et limites : [matrice de compatibilité](docs/PLUGINS-COMPATIBILITE.md).

**Limites :** ces essais sont simulés, sans D6 ; seul OpenAction Counter a été confirmé sur la touche physique. Cela ne garantit pas la compatibilité de tous les plugins ; Gizmos reste sans effet confirmé et les molettes (`Encoder`) ne sont pas simulées. Les Property Inspectors HTML tiers ne sont pas qualifiés comme sûrs ; l'exécution HTML non fiable reste refusée. Ne pas ouvrir d'interface HTML inconnue avant validation de son confinement. Voir [les preuves, modes d'essai et limites](docs/PLUGINS-COMPATIBILITE.md) et [l'architecture](docs/PLUGINS-ARCHITECTURE.md).

Dernier build Windows vérifié (banc MSVC, 19 octobre 2026) : **302 tests applicatifs réussis**, 6 ignorés (essais de plugins tiers opt-in, lancés séparément et réussis), **9 tests MCP réussis**, transports MCP stdio et HTTP réussis. Cette publication du code source n'est pas une distribution binaire ni une certification de compatibilité générale.

## Périphériques pris en charge

- Vérifié sur matériel : FIFINE AmpliGame D6 (15 touches).
- Expérimental, non vérifié sur matériel : modèles Mirabox / AJAZZ et Elgato Stream Deck répertoriés dans le catalogue HID ; compatibilité variable selon le modèle et le protocole.
- Pour participer à la validation : [guide de test des périphériques](docs/TEST-PERIPHERIQUES.md) ; Réglages → Tester mon périphérique… et envoyer le rapport généré.

## Nouveautés 0.5.1

- **Inspecteur complet** : onglets Action / Apparence / Avancé, grand aperçu de la touche, mise en page sûre à 125 % et 150 % (petits écrans compris).
- **Touche selon l'état réel** : titre, icône et couleur suivent une condition (micro coupé = icône rouge, application muette, casque branché), même si l'état change hors de DeckForge.
- **Variables** : blocs Calculer (compteurs, aléatoire) et Texte ; variables enregistrées entre les redémarrages (Réglages → Variables enregistrées) ; `{nom}`, `{date}`, `{heure}`, `{horodatage}`, `{presse_papiers}` dans les textes, liens, arguments et requêtes.
- **Presse-papiers** : copier, lire, sauvegarder / restaurer, modèle « Coller sans mise en forme ».
- **Clavier et souris** : maintenir une touche (relâchée automatiquement en fin de macro, à l'arrêt et à la fermeture), clic / déplacement relatifs à une fenêtre ou au curseur, défilement horizontal.
- **Fenêtres** : activer, réduire, agrandir, déplacer, redimensionner, envoyer sur un autre écran, toujours au premier plan ; « Lancer une application » peut activer celle déjà ouverte.
- **Déclencheurs automatiques** : ouverture / fermeture d'une application, heure fixe, minuteur, branchement d'un périphérique audio (Réglages → Windows et raccourcis).
- **Contrôle d'exécution par touche** : ignorer, relancer ou mettre en file si la macro tourne déjà, anti-rebond.

## Nouveautés 0.5.0

### Nouvelle interface
- **Thème sombre « studio »** : fond bleu nuit, accent bleu électrique, nouveau logo (fenêtre, barre des tâches, zone de notification, exécutable).
- **Navigation à gauche** : profils, pages et carte d'état du FIFINE D6 ; version compacte sur les petites fenêtres.
- **Pavé adaptatif** : les 15 touches suivent la taille de la fenêtre, sans barre de défilement.
- **Bibliothèque en cartes** avec recherche, onglets Actions / Magasin / Mes binds / Plugins.
- **Inspecteur** avec badge d'état, onglets Action et Avancé, « Vider » en rouge pour éviter les erreurs.
- **Plus de clignotement** : rendu wgpu (DirectX 12 / Vulkan).

![Atelier de blocs](docs/workflow.png)

### Blocs natifs (sans AutoHotkey ni programme externe)
- **Volume d'une application** : l'application au premier plan (ou un exécutable précis), en points de pourcentage, muet / rétabli / bascule ; jamais le volume général.
- **Volume d'un périphérique** : micro ou sortie, par défaut ou choisi.
- **Si… sinon** avec 14 conditions : application au premier plan ou ouverte, fenêtre, son en cours, muet, seuil de volume, casque branché, sortie actuelle, variables, et « non / toutes / au moins une ».
- **Attendre que…** une condition, avec délai maximal et bouton Arrêter.
- **Notification Windows** avec variables `{nom}`.
- Aussi : OBS (scènes, enregistrement, stream, caméra virtuelle, replay, sources), Discord, souris, fermer une fenêtre, soundboard, luminosité / veille du D6, UDP, HTTP, touche à deux états.
- **Conversion** des anciens binds `FocusAudio.exe` en blocs natifs (Réglages → À propos), avec sauvegarde.

### Plugins Stream Deck / FIFINE
- Magasin et hôte de plugins introduits dans la 0.5.0 (exe, Node.js, HTML expérimental), page de réglages du plugin, image et titre en direct sur la touche. Les fonctions déjà natives sont proposées en priorité. **État 0.7.0 :** l’exécution HTML non fiable est refusée et les Property Inspectors tiers restent à sécuriser ; voir la section ci-dessus.

![Magasin de plugins](docs/plugins-store.png)

### Déjà dans la 0.5.0
- Annuler / Rétablir (Ctrl+Z / Ctrl+Y), copier / coller / dupliquer une touche, appui long, GIF et vidéos, taille d'icône, profil automatique selon l'application, veille du D6, À propos et vérification des mises à jour, sauvegarde de secours `config.json.bak`.

Détails, tests réalisés et points restant à vérifier : [journal 0.5](docs/CHANGELOG-0.5.md) et [essais manuels 0.5.0](docs/TEST-0.5.0.md).

**Retour en 0.4.3** : garder une copie de `config.json`. La 0.4.3 refuse une configuration contenant un bloc désactivé et ignore (puis efface en réenregistrant) les nouveaux réglages : appui long, tailles, profils associés, veille, animations.

## Nouveautés 0.4.3

**Galerie** est maintenant le grand bouton principal ; le carré **image +** importe un fichier du PC. Le **seau barré** à côté de Couleur active un fond noir sans perdre la couleur choisie. Les transparences PNG/SVG respectent ce choix. Voir [les essais 0.4.3](docs/TEST-0.4.3.md) et [le guide de reprise pour Claude](docs/HANDOFF-CLAUDE.md).

## Correction du rendu 0.4.2

L’aperçu des touches affiche désormais le JPEG réellement destiné au D6, remis à l’endroit : mêmes traits, fonds, titres et images personnalisées. Le cadre en relief reste propre à l’éditeur. Les titres reviennent à la ligne entre les mots et les titres trop longs se terminent par une ellipse. Voir [les essais du correctif](docs/TEST-0.4.2.md).

## Cette version

- Synchronisation automatique : une touche incomplète reste en brouillon, les autres sont utilisables. Les brouillons sont conservés sur disque.
- Interface claire/sombre, représentation du D6 en relief, bibliothèque de 40 binds et galerie des packs locaux FIFINE/Elgato.
- Éditeur de blocs : conditions sur les fenêtres et variables, répétitions, délais, clavier, texte, applications, sites, médias et scripts AutoHotkey v2.
- Basculement entre deux entrées ou sorties audio Windows. Deux cartes permettent de choisir séparément le titre, le périphérique et l’icône de chaque état. Par défaut : casque et enceintes. L’état suit aussi les changements effectués dans Windows.
- Icônes PNG, JPEG et SVG. La sélection dans la galerie a un contour bleu, une coche dessinée et un message de confirmation immédiate.
- Pages et dossiers, enregistrement des combinaisons clavier, raccourcis globaux par page.
- Icône près de l’horloge, option de démarrage Windows en arrière-plan et moteur d’événements indépendant du dessin de la fenêtre.
- 11 outils MCP pour lire les profils, périphériques et icônes, proposer des pages et créer des macros complètes avec blocs. Les propositions attendent une validation dans DeckForge, puis restent modifiables.

## Bien commencer

Glisse une action ou un bind sur une touche. Utilise **Atelier blocs** pour composer une automatisation. Pour l’audio, sélectionne **Basculer l’audio**, puis configure les deux cartes. Les petites images et les boutons **Galerie** ouvrent le choix propre à chaque état.

Dans **Atelier blocs → Créer avec mon IA**, décris ce que tu souhaites et copie la demande pour ton client MCP connecté. Ce champ prépare la demande ; aucun modèle local n’est intégré et aucun abonnement IA n’est configuré automatiquement.

Fermer la fenêtre peut conserver l’application près de l’horloge. Utilise **Réglages → Quitter DeckForge** pour quitter complètement avant une mise à jour.

Les profils restent dans `config.json`, les préférences dans `preferences.json`, à côté de l’exécutable. La mise à jour des binaires ne doit pas remplacer ces fichiers ni le dossier `assets`.

## Validation et limites

Version de test : voir [les contrôles réalisés](docs/VALIDATION.md), [les essais manuels 0.5.0](docs/TEST-0.5.0.md) et [l’essai du plugin tiers sur D6](docs/PLUGINS-COMPATIBILITE.md). La validation 0.7.0 a ouvert une copie isolée de DeckForge avec l’accord de l’utilisateur ; l’application habituelle et les volumes audio n’ont pas été modifiés.

Les plugins tiers sont exécutés par un hôte compatible Stream Deck ; certains plugins peuvent rester partiellement compatibles. L’import de profils ne convertit que les actions reconnues ; voir [compatibilité Elgato](docs/ELGATO.md).

Le retour visuel audio est défini pour une macro contenant un seul basculement audio, y compris après d’autres blocs. Plusieurs basculements dans la même macro ne reçoivent pas d’état visuel supposé. Les variables sont locales à chaque exécution. Exporter en AutoHotkey refuse explicitement les blocs non pris en charge.

## Développement

```text
cargo build --release --locked
DECKFORGE_MCP_EXE=<chemin absolu vers target/release/deckforge-mcp.exe>
cargo test --locked
node tests/mcp-transport.cjs
node tests/mcp-http.cjs
```

La variable active aussi le test intégrant le vrai connecteur stdio à l’application sans fenêtre. Sans elle, ce scénario conditionnel ne lance pas le connecteur. Les tests AutoHotkey installés nécessitent une variable distincte, non activée pendant les travaux en cours de l’utilisateur.

Sources sous MIT ; voir [les licences tierces](THIRD-PARTY-NOTICES.txt). Aucun binaire ni pack propriétaire FIFINE/Elgato n’est redistribué.
