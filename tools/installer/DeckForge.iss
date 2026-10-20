; Installateur Windows de DeckForge (Inno Setup 6.x ou 7.x).
; Compilation : tools\Build-Installer.ps1 -Version 0.8.4   (ou ISCC /DAppVersion=0.8.4 /DSourceDir=...)
; Installation par utilisateur, sans droits administrateur, dans %LOCALAPPDATA%\Programs\DeckForge.
; Les données de l'utilisateur (config.json, preferences.json, variables.json, assets\, plugins\)
; vivent à côté de l'exécutable : elles ne sont ni écrasées à la mise à jour ni supprimées à la
; désinstallation.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef AppNumVersion
  #define AppNumVersion AppVersion
#endif
#ifndef SourceDir
  #define SourceDir "..\..\target\release"
#endif
#ifndef RepoDir
  #define RepoDir "..\.."
#endif
#ifndef OutputDir
  #define OutputDir "..\..\target\release-dist"
#endif

[Setup]
AppId={{7A2E4C1B-5D0F-4E7A-9C3B-DF0E8A61D6F8}
AppName=DeckForge
AppVersion={#AppVersion}
AppVerName=DeckForge {#AppVersion}
AppPublisher=HatiFlash
AppPublisherURL=https://github.com/HatiFlash/DeckForge
AppSupportURL=https://github.com/HatiFlash/DeckForge/issues
AppUpdatesURL=https://github.com/HatiFlash/DeckForge/releases
AppCopyright=© 2026 HatiFlash - licence MIT
VersionInfoVersion={#AppNumVersion}
VersionInfoProductName=DeckForge
VersionInfoDescription=Installateur de DeckForge
DefaultDirName={localappdata}\Programs\DeckForge
DefaultGroupName=DeckForge
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
; Même nom que le mutex d'instance unique de DeckForge (Local\DeckForgeD6).
AppMutex=DeckForgeD6
CloseApplications=yes
RestartApplications=no
SetupIconFile={#RepoDir}\res\branding\deckforge.ico
UninstallDisplayIcon={app}\deckforge.exe
UninstallDisplayName=DeckForge
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
OutputDir={#OutputDir}
OutputBaseFilename=DeckForge-{#AppVersion}-Setup
LicenseFile={#RepoDir}\LICENSE

[Languages]
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "{#SourceDir}\deckforge.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\deckforge-mcp.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoDir}\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#RepoDir}\docs\*.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#RepoDir}\docs\*.png"; DestDir: "{app}\docs"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{autoprograms}\DeckForge"; Filename: "{app}\deckforge.exe"; WorkingDir: "{app}"
Name: "{autoprograms}\Désinstaller DeckForge"; Filename: "{uninstallexe}"
Name: "{autodesktop}\DeckForge"; Filename: "{app}\deckforge.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\deckforge.exe"; Description: "{cm:LaunchProgram,DeckForge}"; WorkingDir: "{app}"; Flags: nowait postinstall skipifsilent
