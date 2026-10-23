# NovyDeck 0.9.3

## English

- The in-app updater now launches its wait using a hidden console inherited by `ping`; the installer remains silent and relaunches NovyDeck. When upgrading **from 0.9.2 to 0.9.3**, the 0.9.2 launcher is still in use and its visible console may appear **one last time**.
- Plugin settings use a smaller, resizable, named window associated with the NovyDeck window when its owner is available. The plugin receives the current interface language instead of always receiving French.
- The settings window is still a separate isolated WebView2 process, for plugin isolation. Reopening settings may create another window; the existing limit of four inspector windows remains. Plugin-supplied HTML controls its own page appearance, so a completely dark page cannot be guaranteed.
- No profiles, macros, or plugin actions are changed by this presentation update.

## Français

- La mise à jour depuis l'application lance maintenant son attente avec une console masquée héritée par `ping` ; l'installateur reste silencieux et relance NovyDeck. Lors du passage **de 0.9.2 à 0.9.3**, c'est encore le code de 0.9.2 qui lance la mise à jour : sa console peut apparaître **une dernière fois**.
- Les réglages du plugin utilisent une fenêtre plus compacte, redimensionnable et nommée, rattachée à celle de NovyDeck lorsque son propriétaire est disponible. Le plugin reçoit la langue réelle de l'interface au lieu de recevoir toujours le français.
- La fenêtre reste un processus WebView2 isolé pour préserver l'isolation du plugin. La rouvrir peut créer une autre fenêtre ; la limite existante de quatre inspecteurs reste en vigueur. Le contenu HTML du plugin contrôle son propre aspect : un fond entièrement sombre ne peut pas être garanti.
- Cette mise à jour de présentation ne modifie ni les profils, ni les macros, ni les actions des plugins.
