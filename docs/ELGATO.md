# Importer depuis Elgato Stream Deck

Dans NovyDeck : **Réglages → Importer depuis Stream Deck…**. Choisis un export
**.streamDeckProfile**, une action exportée dont le manifest est reconnu, ou un
manifest JSON. Un aperçu affiche les conversions et les avertissements. Le bouton
**Importer ce profil** ajoute un nouveau profil ; les touches restent en pause.
Les profils existants ne sont pas remplacés.

Dans le logiciel Elgato, l’export d’un profil est disponible dans les préférences
des profils, menu contextuel **Export**. Voir [le guide officiel](https://docs.elgato.com/streamdeck/sdk/guides/profiles/).

## Ce qui est converti

- Raccourcis Windows dont le code de touche est reconnu et cohérent.
- Texte, avec entrée finale quand elle est indiquée.
- Sites HTTP/HTTPS ; applications désignées par un chemin .exe sans arguments incorporés.
- Pages, dossiers et retour quand leurs identifiants sont résolus.
- Certaines multi-actions et pauses, uniquement si toutes les étapes sont convertibles.
- Images PNG/JPEG valides : réencodées en PNG local, sans charger de code.

Les manifests historiques et les manifests à contrôleurs Keypad sont reconnus.
Un profil de plus de 15 touches signale les positions hors de la grille 5×3.
Si les identifiants de pages ne peuvent pas être résolus, les contenus sont
conservés comme pages indépendantes et leur ordre doit être vérifié.

## Ce qui demande une adaptation

Les plugins Elgato/FIFINE ne sont pas chargés. Les fichiers .streamDeckPlugin,
leurs connexions OAuth, les plugins OBS/Wave Link/Discord, les dials, les toggles,
les scripts embarqués et les raccourcis macOS ne sont pas exécutés directement.
Les actions incompatibles restent vides avec leur titre et un avertissement.
Une multi-action partiellement compatible est laissée entièrement vide.
Les fichiers SVG portant une extension PNG sont signalés et remplacés par l’icône NovyDeck.

Les images importées sont dans **assets/import-…/** près de l’exécutable.
Conserve ce dossier avec config.json pour garder leurs chemins utilisables.
Les exports JSON personnels de NovyDeck n’embarquent pas les images.

Validation locale : le profil Notion 15 touches de
[danewrx/Streamdeck-Profiles](https://github.com/danewrx/Streamdeck-Profiles)
a donné **27 actions et 29 icônes** sur deux contenus ; le rapport demande une
vérification de l’ordre et signale notamment un raccourci incohérent et un chemin
d’ouverture non convertible. Le fichier de référence et ses icônes ne sont pas redistribués.
Des fixtures synthétiques couvrent pages/dossiers, actions inconnues et refus
d’une multi-action partielle. Ce test ne garantit pas tous les exports Elgato.

L’import lit une archive bornée, sans extraction générale : taille, nombre de
fichiers et chemins sont contrôlés. Aucun exécutable du profil n’est extrait ou lancé.
