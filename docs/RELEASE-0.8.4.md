# DeckForge 0.8.4 — icônes IconPark et animations

## Icônes d'interface multi-couleurs
- Nouvelles icônes **IconPark** (ByteDance, licence Apache 2.0) en mode multi-couleur dans toute l'interface : barre supérieure et menu Réglages, bibliothèque d'actions (onglets et chaque action), profils, pages et dossiers, inspecteur, Atelier de blocs (palette, cartes, barre du bas), assistant, Réglages, Assistant IA, plugins, galerie d'icônes, éditeur média, messages d'erreur.
- Une couleur par famille : clavier et souris orange doux, applications bleu, web bleu/turquoise, logique violet/turquoise, médias violet/rose, audio cyan, système bleu acier, OBS corail, plugins vert/bleu, IA violet/bleu, images rose/ambre, navigation à la couleur d'accent choisie. Contours clairs sur thème sombre, foncés sur thème clair.
- Système centralisé (`src/iconset.rs`) : 66 pictogrammes embarqués seulement (45 Ko), chacun rastérisé une seule fois par taille et par thème, net en 125 % et 150 %. Rien n'est téléchargé.
- Le rendu des touches du périphérique et tes images personnalisées ne changent pas.

## Animations
- **Intro au démarrage** : le logo apparaît, puis l'interface se dévoile en fondu (environ 1 s). Elle ne bloque ni la souris ni le clavier.
- **Indicateur de chargement DeckForge** (4 touches qui s'allument tour à tour) pendant les attentes : IA locale, installation, vérification des mises à jour, conversion vidéo, éditeur média.
- **Point « En attente » qui respire** pendant la connexion du périphérique.
- **Fondus** au changement de page, de dossier ou de profil, et au changement d'onglet (bibliothèque, inspecteur, Réglages, Assistant IA). Seule l'opacité varie : rien ne bouge sous la souris.
- Nouveau réglage **« Réduire les animations »** (Réglages → Général) : coupe l'intro et les fondus.

## Corrections
- À 1060 px ou en 150 %, les boutons de l'inspecteur recouvraient « Appui court » : ils passent maintenant à la ligne quand la place manque.

## Compatibilité
Profils et préférences conservés (nouveau réglage facultatif `reduce_motion`, désactivé par défaut). Exécutable : +0,2 Mo.
