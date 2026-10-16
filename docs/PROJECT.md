# DeckForge — synthèse du projet

## But et matériel

Remplacer FIFINE Control Deck par une application Windows native en Rust pour
le FIFINE AmpliGame D6 3142:0060, en donnant la priorité aux macros et à l’automatisation.
Projet privé : [HatiFlash/DeckForge](https://github.com/HatiFlash/DeckForge).
Version : [0.3.0](https://github.com/HatiFlash/DeckForge/releases/tag/v0.3.0).

## Protocole et correction de la touche Lecture

L’installation FIFINE et ses symboles ont été examinés pour recouper la lecture HID.
Rapports Windows de 1024 octets, JPEG 126×126 tournés de 180°, inversion des lignes
uniquement à l’affichage. Entrée ACK … OK : touche octet 9, état octet 10 ; appui 1,
relâchement 0. L’affichage des 15 touches et les appuis 01,08,15 ont été confirmés.
Aucun code ni binaire FIFINE n’est redistribué.

La 0.1 démarrait en diagnostic : les touches sélectionnaient sans exécuter, tandis
que le bouton de test logiciel fonctionnait. La 0.2 a rendu l’activation explicite,
avec connexion automatique, sauvegarde valide et confirmation de synchronisation.
Les anciens appuis et confirmations ne réactivent pas une configuration périmée.

## Version 0.3 — interface et organisation

Organisation inspirée du logiciel Stream Deck : aperçu du D6 à gauche, bibliothèque
à droite et inspecteur redimensionnable en bas. Boîtier en relief, ombres, bords
creusés, contrôles simples et thèmes clair/sombre mémorisés. Icônes vectorielles
pour chaque type d’action ; les LCD reçoivent aussi une icône quand aucune image
personnalisée n’est choisie.

Le magasin local contient 37 binds : média, Windows, édition, navigateur, sites et
workflows. Recherche, filtres, détail et affectation par bouton ou glisser-déposer.
Actions personnalisées combinables, bibliothèque Mes binds, packs JSON, échange
de touches et annulation de la dernière affectation.

Pages et dossiers imbriqués fonctionnent dans l’app et dans le moteur qui traite
les touches du D6. Retour, précédente/suivante, création, renommage et suppression.
Limites : 128 pages/dossiers par profil, 32 niveaux, 128 binds personnels.
Les anciens profils sont lus sans perdre les touches ou images ; le format est
migré vers le schéma 2, avec sauvegarde précédente à l’écriture.

L’enregistrement d’un raccourci capture une combinaison reçue par la fenêtre,
puis remplit l’action. Pas d’écoute en arrière-plan. Les touches physiques sont
suspendues durant cette saisie. Les raccourcis réservés à Windows restent limités
par ce que la fenêtre peut recevoir.

## Import Elgato

Un importeur de profils/actions exportés et manifests JSON propose un aperçu,
puis ajoute un profil avec les touches en pause. Conversion des raccourcis Windows
cohérents, textes, sites, certains lancements et multi-actions, pages/dossiers et
images PNG/JPEG. Rapport des actions non convertibles ; une séquence partielle
n’est jamais exécutée. Les plugins propriétaires et les comptes ne sont pas repris.
Un profil Notion réel a fourni 27 actions et 29 icônes ; ses limites sont documentées
dans [le guide d’import](https://github.com/HatiFlash/DeckForge/blob/main/docs/ELGATO.md).

## Macros et assistant IA

Étapes : pauses, raccourcis, texte Unicode, application/arguments, site, attente de
fenêtre, média/volume, profil et navigation. Répétitions, réorganisation, simulation,
test différé et arrêt Ctrl+Alt+F12. Les saisies visent la fenêtre active ; l’attente
de fenêtre peut la mettre au premier plan.

MCP optionnel, désactivé par défaut : 8 outils, transport stdio et HTTP localhost
authentifié. L’IA lit, simule ou propose ; changements et exécutions réelles attendent
une validation dans DeckForge. Les pages existantes sont adressables par identifiant.
ChatGPT web exige une liaison distante sécurisée adaptée ; aucun compte IA n’est configuré.

## Livraison et validation

Application portable, sources, documentation, notices de licences et empreintes SHA-256.
Les profils personnels, images importées, préférences, clés MCP et SSH sont exclus
du dépôt et des archives. Le code est MIT ; les références protocolaires sont
mirajazz, duby, FifineControlDeck et opendeck-ampgd6, citées dans le README.

46 tests Rust, 18 contrôles MCP, tests des widgets et rendu hors écran des deux thèmes.
Le contrôle de l’écran a été arrêté avec Échap : les derniers changements matériels
de navigation et d’icônes n’ont pas reçu de nouvel essai physique. Endurance et
veille/reprise restent à éprouver. Détails : [validation](https://github.com/HatiFlash/DeckForge/blob/main/docs/VALIDATION.md).
