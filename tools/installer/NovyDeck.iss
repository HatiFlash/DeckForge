; Installateur Windows de NovyDeck (Inno Setup 6.x ou 7.x).
; Compilation : tools\Build-Installer.ps1 -Version 0.8.12   (ou ISCC /DAppVersion=0.8.12 /DSourceDir=...)
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
AppName=NovyDeck
AppVersion={#AppVersion}
AppVerName=NovyDeck {#AppVersion}
AppPublisher=HatiFlash
AppPublisherURL=https://github.com/HatiFlash/NovyDeck
AppSupportURL=https://github.com/HatiFlash/NovyDeck/issues
AppUpdatesURL=https://github.com/HatiFlash/NovyDeck/releases
AppCopyright=© 2026 HatiFlash - licence MIT
VersionInfoVersion={#AppNumVersion}
VersionInfoProductName=NovyDeck
VersionInfoDescription=Installateur de NovyDeck
DefaultDirName={code:InstallDirectory}
DefaultGroupName=NovyDeck
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
SetupIconFile={#RepoDir}\res\novydeck.ico
UninstallDisplayIcon={app}\novydeck.exe
UninstallDisplayName=NovyDeck
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
OutputDir={#OutputDir}
OutputBaseFilename=NovyDeck-{#AppVersion}-Setup
LicenseFile={#RepoDir}\LICENSE

[Languages]
Name: "fr"; MessagesFile: "compiler:Languages\French.isl"
Name: "en"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
Source: "{#SourceDir}\novydeck.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\novydeck-mcp.exe"; DestDir: "{app}"; Flags: ignoreversion
; Legacy command paths stay functional for existing MCP clients and shortcuts.
Source: "{#SourceDir}\novydeck.exe"; DestDir: "{app}"; DestName: "deckforge.exe"; Flags: ignoreversion
Source: "{#SourceDir}\novydeck-mcp.exe"; DestDir: "{app}"; DestName: "deckforge-mcp.exe"; Flags: ignoreversion
Source: "{#RepoDir}\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoDir}\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoDir}\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#RepoDir}\docs\*.md"; DestDir: "{app}\docs"; Flags: ignoreversion
Source: "{#RepoDir}\docs\*.png"; DestDir: "{app}\docs"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#RepoDir}\docs\images\*.png"; DestDir: "{app}\docs\images"; Flags: ignoreversion skipifsourcedoesntexist

Source: "{#RepoDir}\docs\media\*.png"; DestDir: "{app}\docs\media"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#RepoDir}\docs\media\*.mp4"; DestDir: "{app}\docs\media"; Flags: ignoreversion skipifsourcedoesntexist

[InstallDelete]
Type: files; Name: "{autoprograms}\DeckForge.lnk"
Type: files; Name: "{autoprograms}\Désinstaller DeckForge.lnk"
Type: files; Name: "{autodesktop}\DeckForge.lnk"

[Icons]
Name: "{autoprograms}\NovyDeck"; Filename: "{app}\novydeck.exe"; WorkingDir: "{app}"
Name: "{autoprograms}\Désinstaller NovyDeck"; Filename: "{uninstallexe}"
Name: "{autodesktop}\NovyDeck"; Filename: "{app}\novydeck.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\novydeck.exe"; Description: "{cm:LaunchProgram,NovyDeck}"; WorkingDir: "{app}"; Flags: nowait postinstall skipifsilent

[Code]
function InstallDirectory(Param: String): String;
begin
  { Keep the installed location and profiles; only fresh installations choose a new folder. }
  if DirExists(ExpandConstant('{localappdata}\Programs\DeckForge')) then
    Result := ExpandConstant('{localappdata}\Programs\DeckForge')
  else
    Result := ExpandConstant('{localappdata}\Programs\NovyDeck');
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Value: String;
  RunKey: String;
begin
  if CurStep <> ssPostInstall then Exit;
  RunKey := 'Software\Microsoft\Windows\CurrentVersion\Run';
  { Update only the old autorun entry if it pointed at this managed install. }
  if RegQueryStringValue(HKCU, RunKey, 'DeckForge', Value) then
    if Pos(Lowercase(ExpandConstant('{app}\deckforge.exe')), Lowercase(Value)) > 0 then
      RegWriteStringValue(HKCU, RunKey, 'DeckForge',
        '"' + ExpandConstant('{app}\novydeck.exe') + '" --background');
end;
