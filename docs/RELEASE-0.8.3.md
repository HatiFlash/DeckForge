# DeckForge 0.8.3 — interface allégée et IA locale plus sûre

## Interface
- **Un seul « Assistant IA »** : les boutons « Connecter une IA » et « IA : … » de la barre supérieure sont remplacés par un bouton unique qui affiche l'état (« IA : désactivée », « locale », « connectée », « locale + connectée »). La fenêtre Assistant IA a deux onglets : *IA locale (sur ce PC)* et *IA connectée (Claude, ChatGPT…)*.
- **Barre supérieure plus calme** : « Arrêter les macros en cours » n'apparaît que pendant une exécution (Ctrl + Alt + F12 reste toujours actif). Le badge rouge « Aucun périphérique » en double est retiré ; l'état reste sous le logo et dans la carte en bas à gauche, en gris neutre quand tu as choisi de déconnecter. Un clic sur cette carte connecte le périphérique.
- **Réglages en onglets** (Général, Périphérique, Diagnostic) : la fenêtre tient sur un écran de portable (1366 × 768), libellés à gauche et valeurs avec unités (%, min, ms), textes d'aide plus grands.
- **Tester / Simuler clarifiés** : « Simuler » (déroule les actions dans le journal, sans aucun effet) et « Tester pour de vrai » (exécute dans 3 s sur la fenêtre active), comme dans l'Atelier.

## IA locale
- **Mémoire libérée automatiquement** après 10 minutes sans demande (jamais pendant une réponse) ; bouton « Libérer maintenant ». Le moteur redémarre en quelques secondes à la demande suivante.
- **Repli sur le processeur** si le démarrage sur la carte graphique échoue (mémoire vidéo insuffisante, pilote Vulkan absent) ; le mode utilisé est affiché, et un journal `llama-server.log` aide au diagnostic.
- **Installation annulable et reprise** là où elle s'est arrêtée ; l'espace disque est vérifié avant de télécharger ; le téléchargement s'arrête si DeckForge est fermé.
- **« Pas ça ? Demander plutôt à l'IA locale »** : quand les règles intégrées ont répondu (ou posent une question), un clic envoie la demande à l'IA locale.
- Règles enrichies : « colle sans mise en forme » utilise le bloc fiable du catalogue, « une demi-seconde », « deux secondes », « appuie sur Entrée / Échap » sont compris.

### Mesures réelles (banc Windows sans carte graphique)
30 demandes libres, suite de blocs exacte exigée :

| Moteur | Modèle seul | Assistant complet (règles puis IA) | Temps moyen |
|---|---|---|---|
| Intégré (llama.cpp, Qwen2.5 1,5B) | 24/30 | 28/30 (0.8.2 : 26/30) | 1,0 s |
| Ollama 0.40.6 | 23/30 | 29/30 (0.8.2 : 27/30) | 1,9 s |

Repli processeur, coupure impossible pendant une réponse, libération après inactivité et redémarrage : vérifiés sur le banc.

## Compatibilité
Profils et préférences conservés. Aucun changement de format de configuration.
