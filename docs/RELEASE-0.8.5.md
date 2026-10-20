# DeckForge 0.8.5 — interface, pavé SVG et assistant local

## Bibliothèque de plugins
- La liste entière défile dans le panneau droit, y compris les commandes d'importation : fini le bas des cartes inaccessible sur fenêtre compacte.
- Nom et auteur passent à la ligne ; bouton **Importer ce plugin** placé sous chaque carte et détails/actions repliables. La section Audio natif est repliable sans supprimer ses commandes.
- Aucun plugin n'est exécuté par sa simple découverte ou son importation.

## Éditeur de touches
- La vignette et le bouton « Changer l'icône » restent fixes dans la colonne gauche. L'onglet Apparence défile uniquement les propriétés à droite.
- Châssis SVG réactif de 15 touches 5 × 3, icônes natives IconPark par défaut et angles des onglets arrondis. Les icônes et images choisies par l'utilisateur ne sont pas remplacées.

## IA locale (facultative)
- Catalogue de connaissances éditable dans `res/ia/ia-locale.json`, reconnaissance de demandes conditionnelles ou répétitives et résolution des applications Windows connues.
- Propositions présentées en aperçu et insérées uniquement après validation explicite ; simulation sans effet système. Aucun modèle local n'est activé par défaut.
- Banc de demandes (VM) : 28/30 simples et 12/12 complexes sur chacun des deux moteurs. Il ne remplace pas un essai sur le vrai FIFINE D6.

## Vérifications et limites
- Sur la VM Windows : 372 tests réussis, 0 échec (18 cas ignorés, dont essais interactifs/matériels), compilation release et installateur Inno Setup 0.8.5 réussis. Contrôles d'interface à 1060 et 1366 px et test du défilement du formulaire. Publication GitHub à confirmer par la CI de la release.
- La pointe mémoire de 28 Go signalée sur la 0.8.4 n'a pas été reproduite ni attribuée. **Cette version ne prétend pas corriger une fuite mémoire confirmée.**
