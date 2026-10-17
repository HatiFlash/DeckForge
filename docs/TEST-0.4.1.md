# Essais manuels — DeckForge 0.4.1

L’application en cours n’a pas été remplacée pendant que tu travailles.

## Installer quand tu es disponible

1. Quitte complètement DeckForge avec **Réglages → Quitter DeckForge** (la croix peut seulement masquer la fenêtre).
2. Lance **Mettre-a-jour.cmd** depuis le dossier de mise à jour. Le programme refuse de continuer si DeckForge tourne encore, sauvegarde les anciens exécutables et remplace uniquement les binaires et leur documentation. Les profils, préférences et icônes restent dans leur dossier actuel.
3. Ouvre ensuite DeckForge depuis son dossier habituel.

Si tu télécharges l’archive GitHub, extrais-la d’abord ; l’outil te demandera le dossier contenant l’ancienne application.

## 1. Icônes audio par défaut

Sélectionne ton bouton audio. Les deux cartes **Casque** et **Enceintes** doivent être visibles avec leurs propres icônes. Utilise **Par défaut** sur les deux si nécessaire. Bascule avec le D6 : son image et l’aperçu de DeckForge doivent suivre la sortie active, même si une image générale avait été choisie auparavant. Essaie aussi un changement depuis Windows.

## 2. Personnalisation des deux états

Dans la carte Casque, clique sur son image ou **Galerie**, puis choisis une icône. Fais un autre choix pour Enceintes. Chaque état doit conserver sa propre image, y compris après avoir quitté et relancé DeckForge. Les choix intégrés Casque/Enceintes/Microphone sont disponibles en haut de la galerie audio.

## 3. Retour visuel de sélection

Un clic sur une icône doit immédiatement afficher un **contour bleu**, une **coche** et un message indiquant la touche ou l’état modifié. Le changement est déjà appliqué ; **Terminé** ferme la galerie. Vérifie que passer à l’autre état audio sélectionne bien sa propre icône.

## 4. Automatisation par IA

Dans **Atelier blocs → Créer avec mon IA**, décris une petite automatisation puis copie la demande. Active et configure le MCP dans ton client IA en suivant [MCP.md](MCP.md). La proposition doit apparaître dans DeckForge ; vérifie-la puis valide-la. Les blocs doivent être modifiables dans l’Atelier et aucune macro ne doit se lancer à la validation.

La connexion avec un véritable client et compte IA reste à vérifier manuellement : les tests automatisés ont contrôlé le connecteur complet avec un client de test, sans appeler d’IA distante.

## 5. Arrière-plan

Ferme la fenêtre avec la croix, vérifie que les touches restent fonctionnelles puis relance DeckForge : la fenêtre doit se rouvrir sans créer une seconde instance. Cet essai natif n’a pas été relancé pour préserver ton écran.
