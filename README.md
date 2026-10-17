# DeckForge 0.4.3 — version de test Windows

Application Rust indépendante pour le FIFINE AmpliGame D6 (USB 3142:0060), avec macros, pages, dossiers et connecteur MCP optionnel. Le logiciel FIFINE doit être fermé pour laisser DeckForge accéder au D6.

![Interface](docs/interface.png)

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

Version de test : voir [les contrôles réalisés](docs/VALIDATION.md) et [les essais manuels](docs/TEST-0.4.3.md). Cette correction a été développée et vérifiée sans ouvrir d’application sur l’écran de l’utilisateur et sans modifier ses périphériques audio.

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
