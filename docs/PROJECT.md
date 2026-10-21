# NovyDeck — état du projet au 17 octobre 2026

Version 0.4.3 de test. Dépôt : https://github.com/HatiFlash/NovyDeck.

## Ajouts 0.4.3

Galerie devient l’action principale, avec un carré image + pour importer depuis le PC. Le seau barré supprime le fond coloré au profit d’un écran noir et conserve la couleur pour la rétablir. Les PNG/SVG transparents se composent sur le fond choisi. Le nouveau booléen background vaut true pour les profils anciens ; les propositions MCP conservent le réglage quand ce champ est absent. Un dossier de reprise avec sources, binaires, tests et documentation technique est préparé pour Claude, sans clés ni jetons.

## Correction 0.4.2 — aperçu fidèle au LCD

Le retour utilisateur montre que l’ancien aperçu utilisait d’autres symboles et un autre fond que les images LCD. Les deux sorties utilisent désormais le même moteur : l’éditeur affiche le JPEG qui est préparé pour le D6, sans sa rotation de transport. Les images importées occupent la même surface et portent le même titre aux deux endroits. Le cache se renouvelle quand le contenu ou le fichier d’icône change. Les mots des titres ne sont plus coupés tous les douze caractères.

Les 71 tests applicatifs et 3 tests du connecteur passent, avec 18 contrôles de transport MCP. Une comparaison pixel par pixel couvre les 31 cas d’icônes, les dimensions 112/126, les textures réellement utilisées par les widgets, les deux états audio, PNG, SVG et le repli en cas de fichier absent. Le rendu a été vérifié dans les deux thèmes et les deux tailles de fenêtre sans fenêtre native, USB ni changement audio. L’application en cours n’a pas été remplacée.

## Origine et base

Remplacement Windows du logiciel FIFINE AmpliGame D6, construit en Rust avec eframe/egui, HIDAPI et les interfaces audio Windows. Le protocole USB a été étudié à partir de l’appareil, du logiciel FIFINE installé et de références publiques. Les binaires FIFINE ne sont pas intégrés à NovyDeck.

Le D6 utilise des rapports HID de 1 024 octets et des images JPEG 126 × 126. Le décodage des appuis et le repérage des lignes ont été vérifiés lors des premiers essais matériels. Les dernières versions conservent des révisions de synchronisation pour empêcher l’exécution d’un ancien profil.

## Fonctions présentes

Synchronisation et brouillons par touche ; profils, pages et dossiers ; catalogue de 40 binds ; édition et échange des touches ; thèmes clair et sombre ; aperçu en relief ; galerie locale PNG/JPEG/SVG ; raccourcis enregistrés et déclencheurs globaux ; éditeur de blocs avec conditions, boucles et variables ; AutoHotkey v2 facultatif ; basculement audio à deux états ; arrière-plan et option de démarrage Windows ; MCP stdio et HTTP local avec propositions à valider ; import partiel des profils Stream Deck.

## Retour utilisateur et corrections 0.4.1

L’utilisateur a confirmé la mise à jour en direct, le basculement audio et les blocs. Il a signalé une image audio qui restait fixe et l’absence de surbrillance après sélection d’une icône.

Cause audio reproduite dans sa configuration : une image générale du bouton masquait les icônes d’état. La présentation privilégie désormais les images propres à l’état et, à défaut, les deux symboles casque/enceintes. Deux cartes compactes exposent clairement leurs réglages indépendants. Le suivi fonctionne aussi si un délai précède le bloc audio.

La galerie affiche un contour bleu, une coche vectorielle et le nom de la destination. La destination est figée pendant le choix pour éviter de modifier une autre touche. Le même sélecteur permet de personnaliser séparément les deux états audio.

L’Atelier donne accès à une demande rédigée pour une IA connectée au MCP. Un test relie le véritable connecteur stdio à une application simulée, propose une macro audio avec blocs, valide la proposition au travers des widgets et vérifie que les blocs restent modifiables. Aucun modèle local n’a été ajouté.

## Travail sans perturber le PC

À la demande de l’utilisateur, aucune fenêtre native n’a été ouverte et aucun contrôle de son écran n’a été effectué pendant ces corrections. L’application en cours n’a été ni fermée ni remplacée. Les essais ont utilisé des configurations temporaires et un rendu logiciel des widgets.

## Validation restante

L’utilisateur doit encore vérifier les icônes sur son D6 avec la nouvelle version, le choix des deux icônes dans l’interface, la connexion réelle à son compte/client IA et le comportement complet de masquage/réouverture. Les tests logiciels et leurs limites sont détaillés dans [VALIDATION.md](VALIDATION.md).

## Références

- https://github.com/TheBeardofKnowledge/StreamDeck : transfert des plugins/packs depuis Elgato vers FIFINE, sans moteur indépendant.
- https://github.com/4ndv/mirajazz
- https://github.com/fabiosammy/duby
- https://github.com/ZoutMax/FifineControlDeck
- Documentation officielle Windows Core Audio et RegisterHotKey ; SDK Elgato ; documentation officielle AutoHotkey v2.
