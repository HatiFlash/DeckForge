# DeckForge 0.5.0 — version de test Windows

Application Rust indépendante pour le FIFINE AmpliGame D6 (USB 3142:0060), avec macros, pages, dossiers et connecteur MCP optionnel. Le logiciel FIFINE doit être fermé pour laisser DeckForge accéder au D6.

![Interface](docs/interface.png)

## Nouveautés 0.5.0

- **Édition plus sûre** : Annuler / Rétablir sur 50 étapes (Ctrl+Z / Ctrl+Y), copier / coller / dupliquer une touche par clic droit ou Ctrl+C / Ctrl+V, même entre pages et profils.
- **Atelier de blocs repensé** : il s'ouvre en grand à la place du deck ; palette de blocs par catégories avec recherche ; « + Insérer ici » entre deux blocs ; cartes repliables avec une phrase résumée ; dupliquer, désactiver ou retirer un bloc ; blocs incomplets signalés en orange ; six modèles prêts à personnaliser ; mode Simple (champs essentiels et aide) ou Avancé (tous les réglages) ; déroulé lisible de la simulation ; raccourcis Ctrl+D, Suppr, Alt+↑/↓.
- **Appui long** : chaque touche peut avoir une seconde série d'actions déclenchée en la maintenant (seuil réglable, 500 ms par défaut). Sans appui long, la touche réagit toujours immédiatement.
- **Icônes animées** : GIF animés et vidéos (MP4, WebM, MOV, MKV, AVI, converties en GIF à l'import avec FFmpeg, 10 s maximum, sans son). Curseur **Taille de l'icône** (20 à 200 %) et taille du titre par touche.
- **Profil automatique** selon l'application au premier plan (Applications associées dans le menu du profil, interrupteur dans Réglages) et **veille de l'écran** du D6 après inactivité ; le premier appui réveille sans lancer la macro.
- **Fiabilité** : le D6 reste réactif pendant les boîtes « Ouvrir un fichier », sauvegarde de secours `config.json.bak` atomique avec reprise automatique, erreurs de sauvegarde consignées dans `deckforge-error.log`, MCP HTTP concurrent borné.

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

Version de test : voir [les contrôles réalisés](docs/VALIDATION.md) et [les essais manuels](docs/TEST-0.5.0.md). Cette correction a été développée et vérifiée sans ouvrir d’application sur l’écran de l’utilisateur et sans modifier ses périphériques audio.

Les plugins tiers Elgato/FIFINE sont inventoriés, mais leur moteur d’exécution n’est pas encore implémenté. L’import de profils ne convertit que les actions reconnues ; voir [compatibilité Elgato](docs/ELGATO.md).

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
