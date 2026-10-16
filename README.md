# DeckForge 0.2

Application Windows native en **Rust** pour le **FIFINE AmpliGame D6 `3142:0060`**.
Macros, profils, images et accès USB direct, avec une interface claire inspirée
des applications Apple. Aucun compte ni logiciel FIFINE requis.

![Interface](docs/interface.png)

## Démarrer

1. Extrais l’archive portable dans un dossier où tu peux écrire.
2. Quitte FIFINE Control Deck et ouvre `DeckForge.exe`.
3. La connexion est automatique. Attends **D6 connecté** et **Touches actives** :
   les appuis physiques exécutent alors les actions enregistrées.
4. Choisis une touche et compose ses étapes dans l’éditeur à droite.
5. Les modifications valides sont sauvegardées après une courte pause, puis
   envoyées au D6. L’état des touches reste visible dans la barre supérieure.

**Simuler** affiche une itération sans effet sur le PC. **Exécuter dans 3 s**
exécute réellement la macro après un délai initial pour changer de fenêtre.
Raccourcis et texte sont envoyés à la fenêtre active ; l’étape **Attendre une
fenêtre**, avec mise au premier plan, permet de cibler une application.

**Ctrl + Alt + F12** interrompt la macro. **Tout arrêter** l’interrompt et met les
touches en pause. L’app doit rester ouverte ; elle peut être minimisée.

## Fonctions

- 15 touches : titres, couleurs et images PNG/JPEG avec aperçu.
- Macros : pauses, raccourcis, texte Unicode, applications/arguments, sites,
  attente de fenêtre, média/volume et changement de profil.
- Répétitions de 1 à 1000, étapes réordonnables, profils et duplication.
- Sauvegarde automatique valide et copie précédente `.bak`, import/export JSON.
- Journal repliable, diagnostic explicite, reconnexion USB et luminosité.
- Instance unique et détection de l’app FIFINE pour éviter les conflits.
- **Assistant IA MCP** facultatif : stdio et HTTP local protégé, 8 outils,
  propositions à approuver dans l’app, simulation et arrêt.

Pour lancer PowerShell ou AutoHotkey, choisis explicitement l’interpréteur avec
un argument par ligne. N’ajoute pas de guillemets autour des chemins : les
arguments ne sont pas automatiquement réinterprétés par un shell.

## Correction du déclenchement physique

La 0.1 démarrait en diagnostic : un appui sélectionnait la touche sans exécuter
son action, tandis que le bouton de test restait utilisable. En 0.2, les touches
deviennent actives après connexion et confirmation de synchronisation.
Le diagnostic est un choix explicite dans **Réglages du D6**.

Pendant l’édition ou l’envoi USB, l’état explique la suspension temporaire.
Les touches reprennent après sauvegarde et confirmation de la bonne révision.
Une ancienne confirmation USB et les appuis accumulés pendant un dialogue ne
peuvent pas déclencher une configuration devenue obsolète.

## Assistant IA

Active **Assistant IA** dans la barre latérale. **Copier la configuration MCP**
fournit les paramètres du client local. Pour un client HTTP, le même écran lance
`http://127.0.0.1:9846/mcp` et permet de copier sa clé d’accès.

Les modifications proposées et les exécutions réelles attendent une validation
dans DeckForge. Désactiver l’assistant refuse les propositions en attente et
arrête le serveur HTTP démarré par l’app. Un service distant comme ChatGPT web
nécessite un tunnel ou proxy adapté ; aucun endpoint public n’est créé.

Voir [le guide MCP](docs/MCP.md), [la synthèse](docs/PROJECT.md) et
[le rapport de validation](docs/VALIDATION.md).

## Fichiers locaux

`config.json` est enregistré à côté de l’exécutable, avec une copie précédente
`config.json.bak`. Les images restent des chemins absolus ; l’export JSON ne les
embarque pas. Une configuration illisible n’est pas remplacée : importe une
sauvegarde valide pour la récupérer.

`mcp-connection.json` et `mcp-http-token.txt` contiennent les accès locaux MCP.
Ils sont exclus du dépôt et des archives, comme les profils personnels.
Le jeton HTTP persiste entre deux démarrages ; supprimer son fichier, serveur
HTTP arrêté, permet d’en générer un nouveau.

Une macro s’arrête à la première erreur. Les nouveaux appuis sont ignorés pendant
son exécution. L’annulation ne revient pas sur les actions déjà effectuées.

## Matériel et limites

Révision testée : **D6 `3142:0060`, Windows**. La variante `3142:0007` est exclue.
L’affichage des 15 numéros et les appuis **01 → 08 → 15** ont été validés sur le
matériel de l’utilisateur le 16 octobre 2026. Rapports de sortie : **1024 octets** ;
JPEG **126×126**, rotation 180°, inversion des lignes uniquement à l’affichage.
Les paquets `ACK … OK` utilisent `1` à l’appui et `0` au relâchement ; seul `1`
déclenche une macro.

Version expérimentale : endurance, veille/reprise et toutes les actions dans les
applications réelles restent à éprouver. Pas encore d’OBS WebSocket/Discord natif,
mixeur par application, marketplace, import FIFINE, GIF, appui long, variables,
conditions imbriquées, enregistreur de macros, icône de notification ou démarrage
Windows. OBS/Discord peuvent être pilotés par leurs raccourcis configurés.
Windows peut refuser la saisie dans une application plus privilégiée.

## Compiler et tester

Avec Rust stable et les outils C++ Microsoft sur Windows :

```powershell
cargo test --locked
cargo build --release --locked
node tests/mcp-transport.cjs
node tests/mcp-http.cjs
```

Deux exécutables sont produits : `deckforge.exe` et `deckforge-mcp.exe`.
La cible GNU/MinGW fonctionne également. Le workflow GitHub teste Windows MSVC.
Conserve `LICENSE` et `THIRD-PARTY-NOTICES.txt` avec les binaires.

`--diagnostic` génère un diagnostic USB en lecture seule. `--hardware-test`
affiche des numéros et écoute trois appuis ou trois minutes, sans moteur d’actions.
`--ui-smoke` rend une capture et ferme l’app ; `--small` vérifie le format compact,
et `--connect` ajoute la connexion matérielle.

## Références et rétro-ingénierie

[mirajazz](https://github.com/4ndv/mirajazz),
[duby](https://github.com/fabiosammy/duby),
[FifineControlDeck](https://github.com/ZoutMax/FifineControlDeck) et
[opendeck-ampgd6](https://github.com/shugotekitten/opendeck-ampgd6).

L’installation FIFINE et ses symboles ont aussi été examinés pour recouper le
protocole. Aucun binaire FIFINE n’est redistribué. Un exécutable Qt/C++ ne se
convertit pas automatiquement en sources Rust : les comportements utiles sont
réimplémentés et testés progressivement. Les formats de l’original restent une
référence pour les évolutions. Projet indépendant, code applicatif sous licence MIT.
