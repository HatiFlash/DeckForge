# DeckForge 0.8.2 — IA locale facultative pour l'assistant de l'atelier

## Nouveauté : IA locale (facultative, désactivée par défaut)
L'assistant de l'atelier comprend désormais des demandes libres que ses règles intégrées ne reconnaissent pas (« garde cette fenêtre toujours au-dessus », « rappelle-moi de boire de l'eau », « chanson suivante stp »…). Les règles restent prioritaires ; l'IA locale n'est consultée qu'en repli.

- **Tout reste sur le PC** : seules les adresses locales (127.0.0.1, localhost, ::1) sont acceptées, la demande ne quitte jamais l'ordinateur.
- **Aucune exécution automatique** : la réponse du modèle est contrainte par un schéma JSON, filtrée par une liste blanche de blocs (attendre, raccourci, texte, site web, lancer un programme, média, fenêtre, notification), puis montrée en **aperçu** ; rien n'est inséré sans ta validation. Aucun script, aucune commande ni requête réseau ne peut être proposé.
- **Garde-fous** : une fermeture de fenêtre n'est acceptée que si la demande parle de fermer, « le reste / les autres » n'autorise jamais la fermeture de la fenêtre active, un site n'est ouvert que si son nom figure dans la demande, les blocs répétés sont fusionnés.
- **Deux moteurs au choix** (Réglages → IA locale de l'assistant → Configurer…) :
  - **Intégré à DeckForge** : bouton « Installer le moteur et le modèle (≈1,2 Go) ». Téléchargement unique, sur clic uniquement, de llama.cpp b11751 (GitHub) et de Qwen2.5 1,5B Instruct Q4_K_M (Hugging Face, Apache 2.0), versions épinglées et vérifiées par empreinte SHA-256, installés dans `%LOCALAPPDATA%\DeckForge\ai` (conservés par les mises à jour). Le moteur démarre à la première demande, sans fenêtre, et s'arrête avec DeckForge.
  - **Ollama** déjà installé : `ollama pull qwen2.5:1.5b`, adresse `http://127.0.0.1:11434`.
- Mémoire : environ 1,2 Go de mémoire vidéo, ou de RAM sans carte graphique compatible Vulkan.

### Mesures réelles (banc Windows sans carte graphique, processeur seul)
30 demandes libres en français et en anglais, avec fautes, dont 10 sans ressemblance avec les exemples du modèle ; une réponse compte comme juste seulement si la suite de blocs est exacte.

| Moteur | Modèle seul | Assistant complet (règles puis IA) | Temps moyen |
|---|---|---|---|
| Intégré (llama.cpp, Qwen2.5 1,5B) | 24/30 | 26/30 | 1,0 s |
| Ollama 0.40.6 (qwen2.5:1.5b) | 23/30 | 27/30 | 1,9 s |

Les demandes non comprises donnent une réponse vide ou un bloc inoffensif, toujours en aperçu. Avec une carte graphique, les temps sont plus courts.

## Corrections
- Les réglages de l'IA locale s'ouvrent dans une fenêtre dédiée : la fenêtre Réglages ne grandit pas et reste entièrement visible.

## Compatibilité
Profils et préférences existants conservés ; l'IA locale est désactivée par défaut et n'installe rien sans clic.
