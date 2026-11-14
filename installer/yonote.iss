; Inno Setup script for Yonote.
; Build: "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer\yonote.iss
; (after `cargo build --release`).

#define AppName "Yonote"
#define AppVersion "0.2.1"
#define AppPublisher "MooseDevPJS"
#define AppURL "https://github.com/MooseDevPJS/yonote"
#define AppExe "yonote.exe"

[Setup]
AppId={{8F3C2A71-5B9E-4D2A-9C61-0B7E4F2D9A13}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}/issues
AppUpdatesURL={#AppURL}/releases
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
; Per-user install by default: no administrator rights needed.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
LicenseFile=..\LICENSE
OutputDir=Output
OutputBaseFilename=Yonote-{#AppVersion}-setup
SetupIconFile=..\assets\yonote.ico
WizardImageFile=wizard-large.bmp,wizard-large-2x.bmp
WizardSmallImageFile=wizard-small.bmp,wizard-small-2x.bmp
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
ChangesAssociations=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"

[CustomMessages]
english.OpenWith=Add Yonote to "Open with" for Markdown (.md) files
italian.OpenWith=Aggiungi Yonote ad "Apri con" per i file Markdown (.md)
english.Integration=Windows integration:
italian.Integration=Integrazione con Windows:

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "openwith"; Description: "{cm:OpenWith}"; GroupDescription: "{cm:Integration}"

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[InstallDelete]
; Aggiornamento da "Blocks" (stesso AppId): via il vecchio eseguibile e i vecchi collegamenti.
Type: files; Name: "{app}\blocks.exe"
Type: filesandordirs; Name: "{autopf}\Blocks"
Type: files; Name: "{autoprograms}\Blocks.lnk"
Type: files; Name: "{autodesktop}\Blocks.lnk"

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; Vecchie voci di "Apri con" di Blocks.
Root: HKA; Subkey: "Software\Classes\Blocks.Markdown"; Flags: deletekey dontcreatekey
Root: HKA; Subkey: "Software\Classes\Applications\blocks.exe"; Flags: deletekey dontcreatekey
Root: HKA; Subkey: "Software\Classes\.md\OpenWithProgids"; ValueType: none; ValueName: "Blocks.Markdown"; Flags: deletevalue dontcreatekey
Root: HKA; Subkey: "Software\Classes\.markdown\OpenWithProgids"; ValueType: none; ValueName: "Blocks.Markdown"; Flags: deletevalue dontcreatekey
; Same keys that Settings -> "Open with" writes, so the two stay in sync.
Root: HKA; Subkey: "Software\Classes\Yonote.Markdown"; ValueType: string; ValueName: ""; ValueData: "Nota Markdown (Yonote)"; Flags: uninsdeletekey; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\Yonote.Markdown\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"",0"; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\Yonote.Markdown\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#AppName}"; Flags: uninsdeletekey; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".md"; ValueData: ""; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".markdown"; ValueData: ""; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.md\OpenWithProgids"; ValueType: string; ValueName: "Yonote.Markdown"; ValueData: ""; Flags: uninsdeletevalue; Tasks: openwith
Root: HKA; Subkey: "Software\Classes\.markdown\OpenWithProgids"; ValueType: string; ValueName: "Yonote.Markdown"; ValueData: ""; Flags: uninsdeletevalue; Tasks: openwith

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
; Aggiornamento dall'app (installer silenzioso): Yonote si riapre da sola.
Filename: "{app}\{#AppExe}"; Flags: nowait; Check: WizardSilent
