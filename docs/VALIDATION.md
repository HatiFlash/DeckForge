# Validation — DeckForge 0.4.3

## Ajouts 0.4.3

- 74 tests applicatifs et 3 tests du connecteur : succès ; 18 contrôles MCP stdio/HTTP : succès.
- Chargement des anciens profils, sauvegarde du fond désactivé et conservation de la couleur RGB.
- Image noire, transparence PNG et composition alpha partielle vérifiées dans le rendu canonique.
- Le cache se renouvelle après changement du mode de fond.
- La proposition MCP expose le fond à la validation et conserve le réglage quand le champ est omis.
- Galerie et import image + vérifiés par rendu logiciel en clair/sombre, grand format et format compact. Aucun matériel ni fenêtre native utilisé pour ces modifications.

Les essais physiques du nouveau fond restent manuels : [TEST-0.4.3.md](TEST-0.4.3.md).

## Correction du rendu 0.4.2

- 71 tests applicatifs et 3 tests du connecteur : succès. Le vrai connecteur stdio est activé pour le scénario sans fenêtre.
- 7 contrôles MCP stdio et 11 contrôles HTTP : succès.
- Chaque aperçu est comparé au JPEG USB décodé et remis à l’endroit, pour 31 cas d’icônes et deux dimensions.
- Les textures utilisées par les vrais widgets sont comparées pixel par pixel : défauts, audio casque/enceintes, PNG asymétrique, SVG, fichier manquant et navigation.
- Le cache se renouvelle après modification d’une image, d’un titre, d’une couleur ou d’une action.
- Retours à la ligne entre mots et ellipse pour les titres longs ; aucun changement du protocole USB.
- Aperçus logiciels vérifiés en clair/sombre à 1 060 × 720 et 1 380 × 900. Aucun essai sur USB réel ni modification de la sortie audio pendant ce correctif.
- Le connecteur 0.4.1 est compatible avec l’application 0.4.2 ; la mise à jour d’affichage permet de le conserver pendant que Codex est ouvert.

La comparaison sur l’appareil physique reste à effectuer par l’utilisateur après installation : voir [TEST-0.4.2.md](TEST-0.4.2.md).

## Historique 0.4.1

## Vérifications de cette correction

- Compilation Windows GNU en mode release.
- 67 tests de l’application et 3 tests du connecteur : succès.
- 7 contrôles de transport MCP stdio et 11 contrôles HTTP : succès.
- Test de bout en bout activé avec le véritable connecteur stdio : découverte des 11 outils, lecture de périphériques audio simulés, recherche d’icônes simulées, proposition de blocs, absence d’exécution automatique, validation via les vrais widgets et conservation des blocs modifiables.
- Deux JPEG destinés au LCD sont distincts pour casque/enceintes, avec le même titre et une ancienne image statique. Les icônes d’état personnalisées et SVG sont également vérifiées.
- Affectation d’une icône à un seul état, conservation à la sauvegarde, rejet d’une destination devenue obsolète.
- Sélection de la galerie testée en cliquant sur les vrais widgets sans fenêtre : contour, coche, confirmation et conservation de la destination.
- Les deux cartes audio et leurs contrôles sont visibles à 1 060 × 720 et 1 380 × 900. Aperçus produits par le rendu logiciel des widgets.

Les tests n’ouvrent aucune application sur l’écran, n’envoient aucun rapport USB réel et ne changent aucun périphérique audio. Le scénario conditionnel lançant un interpréteur AutoHotkey installé n’a pas été activé pendant cette correction.

## Confirmations matérielles antérieures

L’utilisateur a confirmé que YouTube et Pause s’affichaient sur le D6, puis que la synchronisation, le basculement audio et les blocs fonctionnaient. Un aller-retour audio Hidizs/Logitech avec restauration des trois rôles Windows avait aussi été vérifié avant sa demande de travailler sans perturber son PC.

Ces observations ne valident pas encore le correctif d’icônes 0.4.1 sur le matériel réel.

## À tester manuellement

Voir [TEST-0.4.1.md](TEST-0.4.1.md). La connexion à un véritable compte ChatGPT/Claude et la réouverture complète depuis l’arrière-plan restent à vérifier. Les plugins Elgato tiers ne sont pas exécutables dans cette version.
