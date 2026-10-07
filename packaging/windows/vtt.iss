; Inno Setup script for the Windows installer. Built by scripts/package-windows.ps1:
;   iscc /DAppVersion=0.1.0 packaging\windows\vtt.iss
; Installs per user by default (no admin prompt); the first page offers all users instead.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{BC21B34E-2F7D-46B2-BED7-CC66FDCE9C88}
AppName=vtt
AppVersion={#AppVersion}
AppVerName=vtt {#AppVersion}
AppPublisher=Chris Andino
AppPublisherURL=https://github.com/chris43123/vertical-tabs-terminal
AppSupportURL=https://github.com/chris43123/vertical-tabs-terminal/issues
DefaultDirName={autopf}\vtt
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile=..\..\LICENSE
SetupIconFile=..\icons\vtt.ico
UninstallDisplayIcon={app}\vtt.exe
OutputDir=..\..\dist
OutputBaseFilename=vtt-{#AppVersion}-windows-x86_64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\..\target\release\vtt.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\config.example.toml"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\vtt"; Filename: "{app}\vtt.exe"; AppUserModelID: "vtt"
Name: "{autodesktop}\vtt"; Filename: "{app}\vtt.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\vtt.exe"; Description: "{cm:LaunchProgram,vtt}"; Flags: nowait postinstall skipifsilent
