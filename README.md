# DeckForge 0.3

Application Windows native en **Rust** pour le **FIFINE AmpliGame D6 `3142:0060`**.
Macros, profils, images et accès USB direct, avec une interface inspirée de l’organisation Stream Deck
et des contrôles iOS. Aucun compte ni logiciel FIFINE requis.

![Interface](docs/interface.png)

## Démarrer

1. Extrais l’archive portable dans un dossier où tu peux écrire.
2. Quitte FIFINE Control Deck et ouvre `DeckForge.exe`.
3. La connexion est automatique. Attends **D6 connecté** et **Touches actives** :
   les appuis physiques exécutent alors les actions enregistrées.
4. Choisis une touche et compose ses étapes dans l’inspecteur en bas.
5. Les modifications valides sont sauvegardées après une courte pause, puis
   envoyées au D6. L’état des touches reste visible dans la barre supérieure.

**Simuler** affiche une itération sans effet sur le PC. **Tester dans 3 s**
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


## Bibliothèque, pages et dossiers — version 0.3

L’aperçu du D6 présente un boîtier en relief avec des touches creusées, des ombres
et des reflets discrets. L’inspecteur se trouve sous les touches ; sa séparation
est redimensionnable. **Clair / Sombre** dans la barre supérieure mémorise le thème.
Les icônes distinguent les actions, les macros, les pages, les dossiers et le retour.
Des icônes sont aussi dessinées sur les LCD quand aucune image personnalisée n’est choisie.

- **Actions** : cliquer ajoute une étape à la touche sélectionnée ; on peut aussi
  glisser une action sur une touche. Une touche de navigation conserve une action unique.
- **Magasin** : 37 binds gratuits intégrés, recherche et catégories. Cliquer affiche
  les détails ; **Utiliser sur la touche** ou glisser affecte le bind entier.
- **Mes binds** : enregistre une copie de ta touche, importe/exporte des packs JSON.
  128 binds personnels maximum ; les images restent des chemins locaux.
- Glisser une touche sur une autre **échange** leurs contenus. **Annuler l’affectation**
  restaure la dernière affectation tant qu’aucune autre modification n’a eu lieu.

Dans la barre sous le profil, **+ → Nouvelle page** ajoute une page. Les flèches et
le sélecteur naviguent dans les pages. Si les emplacements 14 et 15 sont vides,
des touches précédente/suivante y sont proposées ; les emplacements occupés sont conservés.
Les mêmes actions sont disponibles dans la bibliothèque pour les affecter ailleurs.

**+ → Dossier sur la touche sélectionnée** crée un dossier. Double-clique sa touche
dans l’aperçu pour l’éditer, ou appuie sur le D6 pour l’ouvrir. La première touche
du dossier est **Retour**. Les sous-dossiers fonctionnent aussi ; le bouton Retour
de l’app reste disponible même si sa touche LCD a été remplacée.
**+ → Gérer pages et dossiers** permet d’ouvrir, renommer et supprimer les contenus.
Limites : 128 pages/dossiers par profil, 32 niveaux de navigation.

Pour un raccourci, choisis **Enregistrer…**, puis effectue la combinaison souhaitée
(ex. Ctrl + Maj + S). La fenêtre capture une combinaison, sans enregistrer en
arrière-plan ; les touches physiques sont suspendues pendant cette saisie.
Les raccourcis réservés à Windows peuvent quitter cette fenêtre ; les touches
qui n’y sont pas reçues ne peuvent pas être capturées. Les symboles dépendent
du clavier Windows. La saisie manuelle reste disponible.

Ce magasin est un catalogue local DeckForge, pas un accès au marketplace Elgato.
Voir [l’import Stream Deck](docs/ELGATO.md) pour les profils exportés compatibles.

## Assistant IA

Ouvre **Réglages → Assistant IA / MCP** et active l’assistant. **Copier la configuration MCP**
fournit les paramètres du client local. Pour un client HTTP, le même écran lance
`http://127.0.0.1:9846/mcp` et permet de copier sa clé d’accès.

Les modifications proposées et les exécutions réelles attendent une validation
dans DeckForge. Désactiver l’assistant refuse les propositions en attente et
arrête le serveur HTTP démarré par l’app. Un service distant comme ChatGPT web
nécessite un tunnel ou proxy adapté ; aucun endpoint public n’est créé.

Voir [le guide MCP](docs/MCP.md), [la synthèse](docs/PROJECT.md) et
[le rapport de validation](docs/VALIDATION.md).

## Fichiers locaux

Le format de configuration 1 est lu et migré vers le format 2 en conservant les touches et images ; la sauvegarde précédente est gardée.

`preferences.json` contient le thème et Mes binds, avec une copie `.bak`.

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
applications réelles restent à éprouver. La capture des raccourcis est locale à la fenêtre, et ne constitue pas un enregistreur global de macros.
Pas encore d’OBS WebSocket/Discord natif,
mixeur par application, plugins de marketplace tiers, import FIFINE, GIF, appui long, variables,
conditions imbriquées, enregistreur de séquences clavier/souris, icône de notification ou démarrage
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
