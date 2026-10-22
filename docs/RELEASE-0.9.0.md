# NovyDeck 0.9.0 — plugin store and plugin actions

English · [Français](#français)

## English
- **Plugin store.** In **Library → Plugins**, the **Plugin store** button comes first. It opens a Play Store-style window: search, category chips, plugin cards, and a detail page with icon, screenshots (click to enlarge), description in your language, actions, requirements and a large **Install** button. The archive is downloaded only after your confirmation and its SHA-256 fingerprint is checked before import. NovyDeck stays offline until you choose **Connect to the store**.
- **Official NovyDeck plugins**, native and without Node.js: **Clock** (clock, timer, stopwatch), **Counter** (counter, quick note, clipboard), **System Monitor** (CPU, RAM, GPU), **Now Playing** (current track title and cover), **OLED Saver** (black screen on the chosen monitor), **Home Assistant** and **Google Assistant Command** (send a written command such as “turn on the living room light” to your Google Home devices, using your own Google Cloud project). Selected open-source plugins (Elgato CPU, BarRaider Stopwatch) are listed after being tested in NovyDeck.
- **Plugin actions in the block editor.** A new **Plugins** palette lists the actions of every installed plugin. Combine actions from different plugins in the same macro, together with delays, conditions and any other block; each plugin block keeps its own settings.
- **Local assistant knows your plugins.** Ask for “the stopwatch from NovyDeck Clock” and the assistant adds the right plugin block. It never invents an action that is not installed.
- **Start minimized.** Under **Start NovyDeck with Windows**, a new option starts NovyDeck minimized near the clock, without showing the window.

Your profiles, media and plugins are kept. Updating from 0.8.14 or 0.8.15: **About → Check for updates**, or run the installer.

Known limits: Google Assistant Command relies on Google's prototype Assistant SDK API, which only returns audio replies and may change; Home Assistant needs your own server and access token. Third-party plugins run with your user rights: install only plugins you trust.

## Français
- **Magasin de plugins.** Dans **Bibliothèque → Plugins**, le bouton **Magasin de plugins** est en premier. Il ouvre une fenêtre façon Play Store : recherche, catégories, cartes de plugins et fiche détaillée avec icône, captures (clic pour agrandir), description dans votre langue, actions, prérequis et un grand bouton **Installer**. L'archive n'est téléchargée qu'après votre confirmation et son empreinte SHA-256 est vérifiée avant l'import. NovyDeck reste hors ligne tant que vous n'avez pas choisi **Se connecter au magasin**.
- **Plugins NovyDeck officiels**, natifs et sans Node.js : **Horloge** (horloge, minuteur, chronomètre), **Compteur** (compteur, note rapide, presse-papiers), **Moniteur système** (CPU, RAM, GPU), **Lecture en cours** (titre et pochette du morceau), **Économiseur OLED** (écran noir sur le moniteur choisi), **Home Assistant** et **Commande Google Assistant** (envoie une commande écrite comme « allume la lumière du salon » à vos appareils Google Home, avec votre propre projet Google Cloud). Des plugins open source sélectionnés (Elgato CPU, BarRaider Stopwatch) sont listés après avoir été testés dans NovyDeck.
- **Actions de plugins dans l'Atelier de blocs.** Une nouvelle palette **Plugins** liste les actions de chaque plugin installé. Combinez des actions de plugins différents dans une même macro, avec des attentes, des conditions et tous les autres blocs ; chaque bloc de plugin garde ses propres réglages.
- **L'assistant local connaît vos plugins.** Demandez « le chronomètre de NovyDeck Clock » et l'assistant ajoute le bon bloc. Il n'invente jamais une action non installée.
- **Démarrer réduit.** Sous **Lancer NovyDeck au démarrage de Windows**, une nouvelle case démarre NovyDeck réduit près de l'horloge, sans afficher la fenêtre.

Vos profils, médias et plugins sont conservés. Depuis la 0.8.14 ou la 0.8.15 : **À propos → Vérifier les mises à jour**, ou l'installateur.

Limites connues : la Commande Google Assistant utilise l'API prototype « Assistant SDK » de Google, qui ne renvoie plus que des réponses audio et peut évoluer ; Home Assistant nécessite votre propre serveur et un jeton d'accès. Les plugins tiers s'exécutent avec vos droits utilisateur : n'installez que des plugins de confiance.
