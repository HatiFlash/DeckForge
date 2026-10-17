# DeckForge — état du projet au 17 octobre 2026

Version 0.4.1 de test. Dépôt privé : https://github.com/HatiFlash/DeckForge.

## Origine et base

Remplacement Windows du logiciel FIFINE AmpliGame D6, construit en Rust avec eframe/egui, HIDAPI et les interfaces audio Windows. Le protocole USB a été étudié à partir de l’appareil, du logiciel FIFINE installé et de références publiques. Les binaires FIFINE ne sont pas intégrés à DeckForge.

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
