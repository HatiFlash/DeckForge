English · [Français](README.fr.md)

<div align="center">
  <img src="res/branding/novydeck-logo.png" width="120" alt="NovyDeck logo">
  <h1>NovyDeck</h1>
  <p>Open-source Windows software for stream decks: keys, macros, media and plugins, without the official software. Tested on the FIFINE AmpliGame D6 and the Elgato Stream Deck.</p>
  <p>
    <a href="https://github.com/HatiFlash/NovyDeck/releases/tag/v0.8.15"><img src="https://img.shields.io/badge/Release-v0.8.15-3478F6.svg" alt="Release v0.8.15"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-22C58B.svg" alt="MIT license"></a>
    <img src="https://img.shields.io/badge/Windows-10%20%2F%2011-555.svg" alt="Windows 10/11">
  </p>
</div>

![NovyDeck demo profile filling all 15 D6 keys](docs/images/novydeck-main.png)

<sub>Actual interface capture rendered off-screen at 2560 × 1600 with the included “NovyDeck Demo / README” profile; no device connected.</sub>

## Presentation video

[![Watch the NovyDeck presentation film](docs/media/novydeck-demo-poster.png)](docs/media/novydeck-demo.mp4)

A 40-second presentation film in 1920 × 1080 at 60 fps with sound, made with Remotion and three.js. It features a 3D D6 deck, actual interface captures and synthesized music. The published film was produced outside this repository.

## Highlights

- **15 customizable LCD keys:** icons, colors, images, GIFs and short videos, with a preview matching the D6 display.
- **Profiles, pages and folders:** automatically switch profiles based on the foreground application.
- **Visual block editor:** build macros without scripts using shortcuts, text, applications, media, windows, conditions, loops and variables; simulate before running for real.
- **Local assistant:** turn a sentence in English or French into previewed blocks. Nothing is inserted without confirmation; optional local AI is off by default.
- **Stream Deck / OpenAction plugins:** import `.streamDeckPlugin` and `.sdPlugin` packages with isolated HTML settings ([compatibility, French](docs/PLUGINS-COMPATIBILITE.md)).
- **MCP server:** work with Claude, Codex or Cursor while approving every proposed change ([MCP guide, French](docs/MCP.md)).
- **Community plugin library:** browse and install plugins shared by the community from [novydeck-plugins](https://github.com/HatiFlash/novydeck-plugins). NovyDeck only connects after you agree, asks for confirmation and verifies each download's SHA-256; community plugins are marked as unverified.
- **Hardware report:** testing another deck brand? **Signaler un problème avec mon deck** (“Report a problem with my deck”) prepares an anonymized report (preview first) and opens a pre-filled GitHub issue; just add your deck model and what goes wrong.

## Compatibility

| Component | Status |
|---|---|
| Windows 10 and 11, 64-bit | Supported; per-user installer or portable ZIP |
| FIFINE AmpliGame D6 (15 keys) | Supported and tested; primary device |
| Elgato Stream Deck | Tested |
| Other Mirabox-based decks (Mirabox, AJAZZ / Stream Dock…) | Expected to work; not tested yet |
| Stream Deck plugins | Unencrypted plugins supported; encrypted Elgato plugins not supported |
| Local AI (llama.cpp or Ollama) | Optional; disabled by default |
| macOS, Linux | Not supported |

NovyDeck is designed to work with any stream deck. So far it has been tested on the **FIFINE AmpliGame D6** and the **Elgato Stream Deck**. Since the D6 is built on Mirabox hardware, other brands based on the same Mirabox platform should also be compatible, but they have not been tested yet — reports are welcome: use **Signaler un problème avec mon deck** (“Report a problem with my deck”) in the app, or open an [issue](https://github.com/HatiFlash/NovyDeck/issues/new?template=hardware-report.yml).

Close FIFINE Control Deck before using the D6: it holds exclusive USB access to the device.

## Quick start / installation

1. Download **`NovyDeck-0.8.15-Setup.exe`** from the [latest release](https://github.com/HatiFlash/NovyDeck/releases/latest) and run it without administrator rights, or extract `NovyDeck-0.8.15-Windows.zip`. A fresh installation uses `%LOCALAPPDATA%\Programs\NovyDeck`.
2. For updates, the installer updates an existing installation while preserving profiles, media and plugins. For a portable installation, close the app and replace the executables without deleting `config.json`, preferences, `assets` or `plugins`.
3. Launch NovyDeck from the Start menu (or the portable folder) and connect the D6. To explore without changing your profiles, choose **Profiles → ••• → Demo profile (15 keys)**. The same command resets the demo profile; you can also delete it like any other profile.
4. Drag an action from the library onto a key, or open the block editor.

## Gallery

| Block editor | Local assistant |
|---|---|
| ![Five-block macro with shortcut properties](docs/images/novydeck-block-editor.png) | ![Preview of a conditional macro proposed by local rules](docs/images/novydeck-assistant.png) |
| Edit the selected block in the right panel. | Preview a conditional action before adding it. |

| Media cropping | Rounded key corners |
|---|---|
| ![Crop preview for a D6 key](docs/images/novydeck-media-crop.png) | ![Before](docs/images/key-corners-before.png) ![After](docs/images/key-corners-after.png) |
| Preview on the key without changing the original file. | Backgrounds now follow the shape of each key. |

| Plugin settings: Elgato | Plugin settings: VolumeController |
|---|---|
| ![Elgato property inspector](docs/pi-product.png) | ![VolumeController property inspector](docs/pi-volume.png) |
| ![SignalRGB settings](docs/pi-signalrgb.png) | ![Elgato counter settings](docs/pi-counter.png) |

The plugin screenshots use UI test data, not installed user plugins.

## Features in detail

### Devices

The **FIFINE AmpliGame D6** is the primary controller, with 15 LCD keys, visual feedback, brightness control and automatic sleep. The **Elgato Stream Deck** has also been tested, using a direct HID driver with key synchronization and state feedback. Other Mirabox-based controllers (Mirabox, AJAZZ / Stream Dock…) share the D6's hardware platform and should work, but have not been tested yet.

### Visual block editor

Compose automations by dragging blocks in the interface, without AutoHotkey or scripts.

![NovyDeck block workflow](docs/workflow.png)

- **Audio:** adjust the foreground app or a specific executable without changing Windows master volume; switch between headset and speakers and immediately update the key icon and label.
- **Streaming and media:** control OBS Studio scenes, recording, streaming, replay buffer, sources and microphone; use Discord, the built-in soundboard and media controls.
- **Logic:** if/then/else blocks draw on 14 real-time conditions, including active window, playing audio, muted microphone, connected headset and global variables; a wait-until block is also available.
- **System and windows:** move or snap windows, bring them forward, simulate held keys with debounce and move the mouse relatively.
- **Variables and triggers:** react to applications opening or closing, scheduled times or audio events; insert dynamic values such as `{heure}`, `{date}` and `{presse_papiers}` into messages and requests.

### Stream Deck and OpenAction plugins

Import `.streamDeckPlugin` and `.sdPlugin` packages from the built-in store, a file or a folder. In **Library → Plugins**, choose **Import this plugin** on a card, or use **Import all**.

![Plugin library card from a UI test](docs/plugins-store.png)

Native C++/Rust/C# executables, Node.js scripts (`@elgato/streamdeck` v2) and plugins with an HTML backend (`Runtime::Html`) are supported. HTML property inspectors run in an isolated WebView2 subprocess (`--isolated-inspector`) behind a restricted loopback proxy. Offline `sdpi-components` v3/v4 and BarRaider EasyPI are included, with dark-theme integration. Settings synchronize both ways (`setSettings`, `setGlobalSettings`) between the UI and hardware. Encrypted Elgato plugins are not supported.

### Plugin store and community library

Open **Library → Plugins** (**Bibliothèque → Plugins** in the French interface). The tab has two parts:

1. **Installed in NovyDeck** (*Installés dans NovyDeck*): plugins already imported from a file, a folder or the built-in catalog. Drag one of their actions onto a key like any other action.
2. **Community** (*Communauté*): a public directory of third-party plugins maintained in [HatiFlash/novydeck-plugins](https://github.com/HatiFlash/novydeck-plugins).

How the community library works:

- NovyDeck stays offline until you click **Connect to the library**; it then downloads only the catalog (`registry.json`) from GitHub.
- Each card shows the name, version, author, license, tested decks and minimum NovyDeck version. Entries are labelled **Community — unverified**.
- **Install…** opens a confirmation with the download address and the expected SHA-256. NovyDeck downloads the archive, checks its fingerprint and only then imports it. A matching fingerprint proves the file is the one listed, not that third-party code is safe: install only plugins you trust.

```mermaid
flowchart LR
  A[Plugin author] -->|pull request| B[novydeck-plugins registry]
  B -->|CI: schema, download, SHA-256, ZIP check| C[Maintainer review and merge]
  C --> D[registry.json]
  D -->|after your consent| E[NovyDeck: Library → Plugins → Community]
  E -->|confirm + SHA-256 verified| F[Plugin imported]
```

**Share your plugin:** publish a `.streamDeckPlugin` (or a `.zip` containing an `.sdPlugin` folder) at a stable HTTPS address, compute its SHA-256 (`certutil -hashfile file SHA256` on Windows), add an entry to `registry.json` following the [schema](https://github.com/HatiFlash/novydeck-plugins/blob/main/schema/registry.schema.json) and open a pull request. CI checks the entry automatically and the maintainer decides on publication. Full guide: [CONTRIBUTING.md](https://github.com/HatiFlash/novydeck-plugins/blob/main/CONTRIBUTING.md). The **Propose a plugin** button in the app opens the same page.

### Media and cropping

Crop images and videos non-destructively before sending them to a key: Fill or Fit/letterbox, 0.5×–3.0× zoom, repositioning and an immediate key preview. Windows Media Foundation decodes short videos without requiring FFmpeg; looping GIFs are supported. The gallery accepts PNG, JPEG, WebP and transparent SVG images as well as animations.

### Reliability and Elgato import

NovyDeck retains the last five healthy rotating backups (`backups/config-*.bak`) and offers guided restoration without overwriting good data with a corrupted file. Import a complete Elgato `.StreamDeckProfilesBackup` archive in one step, with automatic detection and conversion of its profiles. Macro simulation has no unsolicited system side effects.

### MCP integration

The built-in Model Context Protocol server connects to Claude Desktop, Claude Code, Codex CLI or Cursor from the AI toolbar. Assistants can inspect profiles and propose key layouts and macros; every proposed change is previewed and requires explicit approval before it is applied. No JSON configuration editing is required for the one-click connection.

### Interface and rendering

The interface uses hardware-accelerated wgpu rendering (DirectX 12 / Vulkan, with OpenGL fallback). Its key grid adapts to window size and display resolution. LCD keys can display images, looping GIFs and short videos. Profiles can be organized into folders and switched automatically when an application starts.

## Languages

The interface is available in **English** and **French**. NovyDeck follows the Windows language on first start; change it in **Settings → Language**. Each language is a single JSON file in [`res/i18n/`](res/i18n): to add yours, copy `en.json`, translate the values and open a pull request. See the [translation guide](docs/TRANSLATING.md).

## Privacy

NovyDeck works offline. Local AI stays on your PC and is optional and disabled by default; connected AI and network-enabled plugins are activated only with your explicit consent. Proposed assistant actions are previewed before insertion or application.

## Documentation

[Visual design (French)](docs/DESIGN.md) · [MCP and AI (French)](docs/MCP.md) · [Plugin architecture (French)](docs/PLUGINS-ARCHITECTURE.md) · [Plugin compatibility (French)](docs/PLUGINS-COMPATIBILITE.md) · [Elgato import (French)](docs/ELGATO.md) · [Device tests (French)](docs/TEST-PERIPHERIQUES.md) · [Media guide (French)](scripts/media/README.md)

## Latest release

- **0.8.15:** English interface with a language selector (**Settings → Language**) and community translations as simple JSON files; optional update check at startup (asked once); release notes shown before installing an update and a **What's new** window after it.
- **0.8.14:** update from the app: **À propos → Vérifier les mises à jour** (About → Check for updates) checks the latest GitHub release; **Télécharger et installer** (Download and install) verifies the installer's SHA-256, closes NovyDeck, installs over it (profiles, media and plugins kept) and restarts it. Portable copies are pointed to the download page.
- **0.8.13:** the **Basculer l'audio** (audio switch) action can be dropped onto a key before choosing its two devices, which are then set in the inspector; previously it was rejected with “Choisis deux périphériques audio différents”.
- **0.8.12:** community plugin library (**Bibliothèque → Plugins → Communauté**), anonymized hardware report with a pre-filled GitHub issue, and the **Crop** button under the key preview is no longer cut off.
- **0.8.11:** NOVYDECK lettering on the deck chassis, with UI test coverage and updated captures and video.
- **0.8.10:** preserve the user's transparent logo across PNGs and Windows icon sizes; add reproducible resource generation and an alpha-channel regression test.

See [GitHub Releases](https://github.com/HatiFlash/NovyDeck/releases) for full release notes.

## Build from source

NovyDeck is written in Rust. Build and test with locked dependencies; building the Windows installer requires Inno Setup 6 or 7.

```bash
git clone https://github.com/HatiFlash/NovyDeck.git
cd NovyDeck
cargo build --release --locked
cargo test --locked
powershell -ExecutionPolicy Bypass -File tools\Build-Installer.ps1 -Version 0.8.15 -Build
```

## License & credits

NovyDeck is distributed under the [MIT license](LICENSE). The source repository does not include proprietary third-party binaries, protected archives or closed resource packs. Interface icons come from [IconPark](https://github.com/bytedance/IconPark) (ByteDance), licensed under Apache 2.0; only the icons in use are bundled and recolored for NovyDeck ([notice](res/iconpark/NOTICE.md), [license](res/iconpark/LICENSE)). See [THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt) for notices covering open-source dependencies and bundled libraries.
